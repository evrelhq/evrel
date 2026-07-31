//! JSX expression lowering.

use evrel_js_ir::{
    JsString, JsxAttribute as IrJsxAttribute, JsxAttributeName as IrJsxAttributeName,
    JsxAttributeValue as IrJsxAttributeValue, JsxChild as IrJsxChild,
    JsxElementName as IrJsxElementName, JsxElementOp, JsxFragmentOp,
    JsxMemberBase as IrJsxMemberBase, OperationKind, ValueId,
};
use oxc_ast::ast::{
    JSXAttribute, JSXAttributeItem, JSXAttributeName, JSXAttributeValue, JSXChild, JSXElement,
    JSXElementName, JSXFragment, JSXMemberExpression, JSXMemberExpressionObject,
};
use oxc_span::GetSpan;

use crate::{FrontendError, lower::FunctionLowerer};

use super::{identifier::lower_identifier, lower_expression};

struct LoweredMemberName {
    base: IrJsxMemberBase,
    properties: Vec<Box<str>>,
    reference: Option<ValueId>,
}

/// Lowers a JSX element while preserving its source-level structure.
pub(super) fn lower_jsx_element(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    element: &JSXElement<'_>,
) -> Result<ValueId, FrontendError> {
    let (name, reference) = lower_element_name(lowerer, &element.opening_element.name)?;
    let attributes = lower_attributes(lowerer, &element.opening_element.attributes)?;
    let children = lower_children(lowerer, &element.children)?;
    let operands = reference.into_iter();

    Ok(lowerer.emit_value(
        OperationKind::JsxElement(JsxElementOp::new(name, attributes, children)),
        operands,
    ))
}

/// Lowers a JSX fragment while preserving its source-level children.
pub(super) fn lower_jsx_fragment(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    fragment: &JSXFragment<'_>,
) -> Result<ValueId, FrontendError> {
    let children = lower_children(lowerer, &fragment.children)?;

    Ok(lowerer.emit_value(OperationKind::JsxFragment(JsxFragmentOp::new(children)), []))
}

fn lower_element_name(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    name: &JSXElementName<'_>,
) -> Result<(IrJsxElementName, Option<ValueId>), FrontendError> {
    match name {
        JSXElementName::Identifier(identifier) => Ok((
            IrJsxElementName::Intrinsic(identifier.name.as_str().into()),
            None,
        )),

        JSXElementName::IdentifierReference(identifier) => Ok((
            IrJsxElementName::Reference,
            Some(lower_identifier(lowerer, identifier)?),
        )),

        JSXElementName::NamespacedName(name) => Ok((
            IrJsxElementName::Namespaced {
                namespace: name.namespace.name.as_str().into(),
                name: name.name.name.as_str().into(),
            },
            None,
        )),

        JSXElementName::MemberExpression(member) => {
            let member = lower_member_name(lowerer, member)?;

            Ok((
                IrJsxElementName::Member {
                    base: member.base,
                    properties: member.properties.into_boxed_slice(),
                },
                member.reference,
            ))
        }

        JSXElementName::ThisExpression(_) => Ok((IrJsxElementName::This, None)),
    }
}

fn lower_member_name(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    member: &JSXMemberExpression<'_>,
) -> Result<LoweredMemberName, FrontendError> {
    let mut lowered = match &member.object {
        JSXMemberExpressionObject::IdentifierReference(identifier) => LoweredMemberName {
            base: IrJsxMemberBase::Reference,
            properties: Vec::new(),
            reference: Some(lower_identifier(lowerer, identifier)?),
        },

        JSXMemberExpressionObject::MemberExpression(member) => lower_member_name(lowerer, member)?,

        JSXMemberExpressionObject::ThisExpression(_) => LoweredMemberName {
            base: IrJsxMemberBase::This,
            properties: Vec::new(),
            reference: None,
        },
    };

    lowered
        .properties
        .push(member.property.name.as_str().into());

    Ok(lowered)
}

fn lower_attributes(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    attributes: &[JSXAttributeItem<'_>],
) -> Result<Vec<IrJsxAttribute>, FrontendError> {
    attributes
        .iter()
        .map(|attribute| match attribute {
            JSXAttributeItem::Attribute(attribute) => lower_attribute(lowerer, attribute),

            JSXAttributeItem::SpreadAttribute(spread) => {
                let expression = lowerer.build_expression_region(|lowerer| {
                    lower_expression(lowerer, &spread.argument)
                })?;

                Ok(IrJsxAttribute::Spread { expression })
            }
        })
        .collect()
}

fn lower_attribute(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    attribute: &JSXAttribute<'_>,
) -> Result<IrJsxAttribute, FrontendError> {
    let name = match &attribute.name {
        JSXAttributeName::Identifier(identifier) => {
            IrJsxAttributeName::Identifier(identifier.name.as_str().into())
        }

        JSXAttributeName::NamespacedName(name) => IrJsxAttributeName::Namespaced {
            namespace: name.namespace.name.as_str().into(),
            name: name.name.name.as_str().into(),
        },
    };
    let value = attribute
        .value
        .as_ref()
        .map(|value| lower_attribute_value(lowerer, value))
        .transpose()?;

    Ok(IrJsxAttribute::Named { name, value })
}

fn lower_attribute_value(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    value: &JSXAttributeValue<'_>,
) -> Result<IrJsxAttributeValue, FrontendError> {
    match value {
        JSXAttributeValue::StringLiteral(literal) => Ok(IrJsxAttributeValue::String(
            JsString::new(literal.value.as_str(), literal.lone_surrogates),
        )),

        JSXAttributeValue::ExpressionContainer(container) => {
            let expression = container
                .expression
                .as_expression()
                .ok_or(FrontendError::EmptyJsxAttributeExpression)?;
            let expression =
                lowerer.build_expression_region(|lowerer| lower_expression(lowerer, expression))?;

            Ok(IrJsxAttributeValue::Expression { expression })
        }

        JSXAttributeValue::Element(element) => {
            let expression = lowerer.build_expression_region(|lowerer| {
                lowerer.with_span(element.span(), |lowerer| {
                    lower_jsx_element(lowerer, element)
                })
            })?;

            Ok(IrJsxAttributeValue::Element { expression })
        }

        JSXAttributeValue::Fragment(fragment) => {
            let expression = lowerer.build_expression_region(|lowerer| {
                lowerer.with_span(fragment.span(), |lowerer| {
                    lower_jsx_fragment(lowerer, fragment)
                })
            })?;

            Ok(IrJsxAttributeValue::Fragment { expression })
        }
    }
}

fn lower_children(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    children: &[JSXChild<'_>],
) -> Result<Vec<IrJsxChild>, FrontendError> {
    let mut lowered = Vec::with_capacity(children.len());

    for child in children {
        match child {
            JSXChild::Text(text) => {
                lowered.push(IrJsxChild::Text(text.value.as_str().into()));
            }

            JSXChild::Element(element) => {
                let expression = lowerer.build_expression_region(|lowerer| {
                    lowerer.with_span(element.span(), |lowerer| {
                        lower_jsx_element(lowerer, element)
                    })
                })?;

                lowered.push(IrJsxChild::Element { expression });
            }

            JSXChild::Fragment(fragment) => {
                let expression = lowerer.build_expression_region(|lowerer| {
                    lowerer.with_span(fragment.span(), |lowerer| {
                        lower_jsx_fragment(lowerer, fragment)
                    })
                })?;

                lowered.push(IrJsxChild::Fragment { expression });
            }

            JSXChild::ExpressionContainer(container) => {
                let Some(expression) = container.expression.as_expression() else {
                    continue;
                };
                let expression = lowerer
                    .build_expression_region(|lowerer| lower_expression(lowerer, expression))?;

                lowered.push(IrJsxChild::Expression { expression });
            }

            JSXChild::Spread(spread) => {
                let expression = lowerer.build_expression_region(|lowerer| {
                    lower_expression(lowerer, &spread.expression)
                })?;

                lowered.push(IrJsxChild::Spread { expression });
            }
        }
    }

    Ok(lowered)
}
