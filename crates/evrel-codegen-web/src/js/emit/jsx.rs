//! JSX syntax emission.

use evrel_js_ir::{
    JsFunctionIr, JsxAttribute, JsxAttributeName, JsxAttributeValue, JsxChild, JsxElementName,
    JsxElementOp, JsxFragmentOp, JsxMemberBase, OperationId, ValueId,
};
use oxc_allocator::{Box as ArenaBox, GetAllocator, Vec as ArenaVec};
use oxc_ast::ast::{
    AssignmentTarget, Expression, JSXAttributeItem as AstJsxAttributeItem,
    JSXAttributeName as AstJsxAttributeName, JSXAttributeValue as AstJsxAttributeValue,
    JSXChild as AstJsxChild, JSXClosingElement, JSXClosingFragment, JSXElement,
    JSXElementName as AstJsxElementName, JSXExpression, JSXFragment, JSXIdentifier,
    JSXMemberExpressionObject, JSXOpeningElement, JSXOpeningFragment, TSTypeParameterInstantiation,
};
use oxc_span::SPAN;
use oxc_syntax::operator::AssignmentOperator;

use crate::{
    JsCodegenError,
    js::plan::{JsFunctionPlan, JsValueRepresentation},
};

use super::{FunctionEmission, region::emit_expression_region, value::emit_value_expression};

pub(crate) fn emit_jsx_element_expression<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    operation: OperationId,
    element: &JsxElementOp,
    operands: &[ValueId],
) -> Result<Expression<'ast>, JsCodegenError> {
    let container_local = jsx_container_local(emission.function, emission.plan, operation)?;
    JsxEmitter {
        emission,
        container_local,
    }
    .emit_element(operation, element, operands)
}

pub(crate) fn emit_jsx_fragment_expression<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    operation: OperationId,
    fragment: &JsxFragmentOp,
    operands: &[ValueId],
) -> Result<Expression<'ast>, JsCodegenError> {
    let container_local = jsx_container_local(emission.function, emission.plan, operation)?;
    JsxEmitter {
        emission,
        container_local,
    }
    .emit_fragment(operation, fragment, operands)
}

struct JsxEmitter<'emit, 'ast> {
    emission: FunctionEmission<'emit, 'ast>,
    container_local: &'emit str,
}

impl<'ast> JsxEmitter<'_, 'ast> {
    fn emit_element(
        &self,
        operation: OperationId,
        element: &JsxElementOp,
        operands: &[ValueId],
    ) -> Result<Expression<'ast>, JsCodegenError> {
        let expected_operands = usize::from(element.reference_operand_index().is_some());

        if operands.len() != expected_operands {
            return Err(JsCodegenError::MalformedOperation { operation });
        }

        let opening_name = self.emit_element_name(element.name(), operands)?;
        let closing_name = self.emit_element_name(element.name(), operands)?;
        let attributes = self.emit_attributes(element.attributes())?;
        let children = self.emit_children(element.children())?;
        let has_children = !children.is_empty();
        let opening = JSXOpeningElement::boxed(
            SPAN,
            opening_name,
            None::<ArenaBox<'ast, TSTypeParameterInstantiation<'ast>>>,
            attributes,
            self.emission.builder,
        );
        let closing = has_children
            .then(|| JSXClosingElement::boxed(SPAN, closing_name, self.emission.builder));

        Ok(Expression::JSXElement(JSXElement::boxed(
            SPAN,
            opening,
            children,
            closing,
            self.emission.builder,
        )))
    }

    fn emit_fragment(
        &self,
        operation: OperationId,
        fragment: &JsxFragmentOp,
        operands: &[ValueId],
    ) -> Result<Expression<'ast>, JsCodegenError> {
        if !operands.is_empty() {
            return Err(JsCodegenError::MalformedOperation { operation });
        }

        Ok(Expression::JSXFragment(JSXFragment::boxed(
            SPAN,
            JSXOpeningFragment::new(SPAN, self.emission.builder),
            self.emit_children(fragment.children())?,
            JSXClosingFragment::new(SPAN, self.emission.builder),
            self.emission.builder,
        )))
    }

    fn emit_element_name(
        &self,
        name: &JsxElementName,
        operands: &[ValueId],
    ) -> Result<AstJsxElementName<'ast>, JsCodegenError> {
        match name {
            JsxElementName::Intrinsic(name) => Ok(AstJsxElementName::new_identifier(
                SPAN,
                self.emission.builder.allocator().alloc_str(name),
                self.emission.builder,
            )),

            JsxElementName::Reference => {
                let [reference] = operands else {
                    unreachable!("JSX element operand count was validated")
                };

                match self.emit_value(*reference)? {
                    Expression::Identifier(identifier) => {
                        Ok(AstJsxElementName::IdentifierReference(identifier))
                    }
                    _ => Err(JsCodegenError::UnsupportedValue { value: *reference }),
                }
            }

            JsxElementName::Member { base, properties } => {
                let mut object = match base {
                    JsxMemberBase::Reference => {
                        let [reference] = operands else {
                            unreachable!("JSX element operand count was validated")
                        };

                        match self.emit_value(*reference)? {
                            Expression::Identifier(identifier) => {
                                JSXMemberExpressionObject::IdentifierReference(identifier)
                            }
                            _ => {
                                return Err(JsCodegenError::UnsupportedValue { value: *reference });
                            }
                        }
                    }
                    JsxMemberBase::This => {
                        JSXMemberExpressionObject::new_this_expression(SPAN, self.emission.builder)
                    }
                };

                let (last, prefix) = properties
                    .split_last()
                    .expect("JSX member names are validated by the IR");

                for property in prefix {
                    object = JSXMemberExpressionObject::new_member_expression(
                        SPAN,
                        object,
                        self.emit_identifier(property),
                        self.emission.builder,
                    );
                }

                Ok(AstJsxElementName::new_member_expression(
                    SPAN,
                    object,
                    self.emit_identifier(last),
                    self.emission.builder,
                ))
            }

            JsxElementName::Namespaced { namespace, name } => {
                Ok(AstJsxElementName::new_namespaced_name(
                    SPAN,
                    self.emit_identifier(namespace),
                    self.emit_identifier(name),
                    self.emission.builder,
                ))
            }

            JsxElementName::This => Ok(AstJsxElementName::new_this_expression(
                SPAN,
                self.emission.builder,
            )),
        }
    }

    fn emit_attributes(
        &self,
        attributes: &[JsxAttribute],
    ) -> Result<ArenaVec<'ast, AstJsxAttributeItem<'ast>>, JsCodegenError> {
        let mut emitted = ArenaVec::with_capacity_in(attributes.len(), self.emission.builder);

        for attribute in attributes {
            emitted.push(match attribute {
                JsxAttribute::Named { name, value } => AstJsxAttributeItem::new_attribute(
                    SPAN,
                    self.emit_attribute_name(name),
                    value
                        .as_ref()
                        .map(|value| self.emit_attribute_value(value))
                        .transpose()?,
                    self.emission.builder,
                ),
                JsxAttribute::Spread { expression } => AstJsxAttributeItem::new_spread_attribute(
                    SPAN,
                    self.emit_region(*expression)?,
                    self.emission.builder,
                ),
            });
        }

        Ok(emitted)
    }

    fn emit_attribute_name(&self, name: &JsxAttributeName) -> AstJsxAttributeName<'ast> {
        match name {
            JsxAttributeName::Identifier(name) => AstJsxAttributeName::new_identifier(
                SPAN,
                self.emission.builder.allocator().alloc_str(name),
                self.emission.builder,
            ),
            JsxAttributeName::Namespaced { namespace, name } => {
                AstJsxAttributeName::new_namespaced_name(
                    SPAN,
                    self.emit_identifier(namespace),
                    self.emit_identifier(name),
                    self.emission.builder,
                )
            }
        }
    }

    fn emit_attribute_value(
        &self,
        value: &JsxAttributeValue,
    ) -> Result<AstJsxAttributeValue<'ast>, JsCodegenError> {
        match value {
            JsxAttributeValue::String(value) => Ok(
                AstJsxAttributeValue::new_string_literal_with_lone_surrogates(
                    SPAN,
                    self.emission.builder.allocator().alloc_str(value.as_str()),
                    None,
                    value.has_lone_surrogates(),
                    self.emission.builder,
                ),
            ),
            JsxAttributeValue::Expression { expression } => {
                Ok(AstJsxAttributeValue::new_expression_container(
                    SPAN,
                    self.emit_container_expression(self.emit_region(*expression)?),
                    self.emission.builder,
                ))
            }
            JsxAttributeValue::Element { expression } => {
                let expression = self.emit_region(*expression)?;
                match expression {
                    Expression::JSXElement(element) => Ok(AstJsxAttributeValue::Element(element)),
                    expression => Ok(AstJsxAttributeValue::new_expression_container(
                        SPAN,
                        self.emit_container_expression(expression),
                        self.emission.builder,
                    )),
                }
            }
            JsxAttributeValue::Fragment { expression } => {
                let expression = self.emit_region(*expression)?;
                match expression {
                    Expression::JSXFragment(fragment) => {
                        Ok(AstJsxAttributeValue::Fragment(fragment))
                    }
                    expression => Ok(AstJsxAttributeValue::new_expression_container(
                        SPAN,
                        self.emit_container_expression(expression),
                        self.emission.builder,
                    )),
                }
            }
        }
    }

    fn emit_children(
        &self,
        children: &[JsxChild],
    ) -> Result<ArenaVec<'ast, AstJsxChild<'ast>>, JsCodegenError> {
        let mut emitted = ArenaVec::with_capacity_in(children.len(), self.emission.builder);

        for child in children {
            emitted.push(match child {
                JsxChild::Text(text) => AstJsxChild::new_text(
                    SPAN,
                    self.emission.builder.allocator().alloc_str(text),
                    None,
                    self.emission.builder,
                ),
                JsxChild::Expression { expression } => AstJsxChild::new_expression_container(
                    SPAN,
                    self.emit_container_expression(self.emit_region(*expression)?),
                    self.emission.builder,
                ),
                JsxChild::Spread { expression } => AstJsxChild::new_spread(
                    SPAN,
                    self.emit_region(*expression)?,
                    self.emission.builder,
                ),
                JsxChild::Element { expression } => {
                    let expression = self.emit_region(*expression)?;
                    match expression {
                        Expression::JSXElement(element) => AstJsxChild::Element(element),
                        expression => AstJsxChild::new_expression_container(
                            SPAN,
                            self.emit_container_expression(expression),
                            self.emission.builder,
                        ),
                    }
                }
                JsxChild::Fragment { expression } => {
                    let expression = self.emit_region(*expression)?;
                    match expression {
                        Expression::JSXFragment(fragment) => AstJsxChild::Fragment(fragment),
                        expression => AstJsxChild::new_expression_container(
                            SPAN,
                            self.emit_container_expression(expression),
                            self.emission.builder,
                        ),
                    }
                }
            });
        }

        Ok(emitted)
    }

    fn emit_identifier(&self, name: &str) -> JSXIdentifier<'ast> {
        JSXIdentifier::new(
            SPAN,
            self.emission.builder.allocator().alloc_str(name),
            self.emission.builder,
        )
    }

    fn emit_container_expression(&self, expression: Expression<'ast>) -> JSXExpression<'ast> {
        JSXExpression::from(match expression {
            expression @ Expression::SequenceExpression(_) => {
                Expression::new_assignment_expression(
                    SPAN,
                    AssignmentOperator::Assign,
                    AssignmentTarget::new_assignment_target_identifier(
                        SPAN,
                        self.emission
                            .builder
                            .allocator()
                            .alloc_str(self.container_local),
                        self.emission.builder,
                    ),
                    expression,
                    self.emission.builder,
                )
            }
            expression => expression,
        })
    }

    fn emit_value(&self, value: ValueId) -> Result<Expression<'ast>, JsCodegenError> {
        emit_value_expression(
            self.emission.builder,
            self.emission.function,
            self.emission.plan,
            value,
        )
    }

    fn emit_region(
        &self,
        region: evrel_js_ir::RegionId,
    ) -> Result<Expression<'ast>, JsCodegenError> {
        emit_expression_region(
            self.emission.builder,
            self.emission.module,
            self.emission.output_plan,
            self.emission.function,
            self.emission.plan,
            region,
        )
    }
}

fn jsx_container_local<'a>(
    function: &JsFunctionIr,
    plan: &'a JsFunctionPlan,
    operation_id: OperationId,
) -> Result<&'a str, JsCodegenError> {
    function
        .operation(operation_id)
        .ok_or(JsCodegenError::UnknownOperation {
            operation: operation_id,
        })?;
    let [result] = plan.operation(operation_id).result_destinations() else {
        return Err(JsCodegenError::MalformedOperation {
            operation: operation_id,
        });
    };
    let Some(JsValueRepresentation::Temporary(local)) = plan.value(*result) else {
        return Err(JsCodegenError::UnsupportedValue { value: *result });
    };

    plan.local_name(local)
        .ok_or(JsCodegenError::UnsupportedValue { value: *result })
}
