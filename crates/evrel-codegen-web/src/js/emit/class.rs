//! Conservative JavaScript class-expression emission.

use evrel_js_ir::{
    ClassElement as IrClassElement, ClassElementKey, ClassFieldPlacement, ClassMethodKind,
    ClassMethodPlacement, CreateClassOp, FunctionId, FunctionKind, FunctionMode, JsFunctionIr,
    JsModuleIr,
};
use oxc_allocator::{Box as ArenaBox, GetAllocator, Vec as ArenaVec};
use oxc_ast::{
    AstBuilder,
    ast::{
        BindingIdentifier, ClassBody, ClassElement, ClassType, Expression, FormalParameterKind,
        FormalParameterRest, FormalParameters, FunctionBody, MethodDefinitionKind,
        MethodDefinitionType, PropertyDefinitionType, PropertyKey as AstPropertyKey, Statement,
        TSTypeAnnotation, TSTypeParameterDeclaration, TSTypeParameterInstantiation,
    },
};
use oxc_span::SPAN;

use crate::{
    JsCodegenError,
    js::plan::{JsFunctionPlan, JsModulePlan},
};

use super::{
    function::{emit_function_body, emit_function_node},
    object::emit_static_object_key,
    region::emit_expression_region,
};

pub(crate) fn emit_class_expression<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    output_plan: &JsModulePlan,
    function: &JsFunctionIr,
    plan: &JsFunctionPlan,
    class: &CreateClassOp,
) -> Result<Expression<'ast>, JsCodegenError> {
    let name = class
        .self_binding()
        .map(|binding| {
            let name = plan
                .binding_name(binding)
                .ok_or(JsCodegenError::UnknownBinding { binding })?;

            Ok(BindingIdentifier::new(
                SPAN,
                builder.allocator().alloc_str(name),
                builder,
            ))
        })
        .transpose()?;
    let super_class = class
        .super_class()
        .map(|region| emit_expression_region(builder, module, output_plan, function, plan, region))
        .transpose()?;
    let mut elements = ArenaVec::with_capacity_in(class.elements().len(), builder);

    for element in class.elements() {
        elements.push(emit_class_element(
            builder,
            module,
            output_plan,
            function,
            plan,
            element,
        )?);
    }

    Ok(Expression::new_class_expression(
        SPAN,
        ClassType::ClassExpression,
        ArenaVec::new_in(builder),
        name,
        None::<ArenaBox<'ast, TSTypeParameterDeclaration<'ast>>>,
        super_class,
        None::<ArenaBox<'ast, TSTypeParameterInstantiation<'ast>>>,
        ArenaVec::new_in(builder),
        ClassBody::boxed(SPAN, elements, builder),
        false,
        false,
        builder,
    ))
}

fn emit_class_element<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    output_plan: &JsModulePlan,
    function: &JsFunctionIr,
    plan: &JsFunctionPlan,
    element: &IrClassElement,
) -> Result<ClassElement<'ast>, JsCodegenError> {
    match element {
        IrClassElement::Method(method) => {
            let expected_kind = match method.kind() {
                ClassMethodKind::Constructor => FunctionKind::ClassConstructor,
                ClassMethodKind::Method | ClassMethodKind::Getter | ClassMethodKind::Setter => {
                    FunctionKind::ClassMethod
                }
            };
            let method_ir =
                module
                    .function(method.function())
                    .ok_or(JsCodegenError::UnknownFunction {
                        function: method.function(),
                    })?;

            if method_ir.kind() != expected_kind {
                return Err(JsCodegenError::InvalidFunctionKind {
                    function: method.function(),
                });
            }

            let (key, computed) =
                emit_class_key(builder, module, output_plan, function, plan, method.key())?;
            let kind = match method.kind() {
                ClassMethodKind::Constructor => MethodDefinitionKind::Constructor,
                ClassMethodKind::Method => MethodDefinitionKind::Method,
                ClassMethodKind::Getter => MethodDefinitionKind::Get,
                ClassMethodKind::Setter => MethodDefinitionKind::Set,
            };

            Ok(ClassElement::new_method_definition(
                SPAN,
                MethodDefinitionType::MethodDefinition,
                ArenaVec::new_in(builder),
                key,
                emit_function_node(builder, module, output_plan, method.function(), None)?,
                kind,
                computed,
                method.placement() == ClassMethodPlacement::Static,
                false,
                false,
                None,
                builder,
            ))
        }

        IrClassElement::Field(field) => {
            let (key, computed) =
                emit_class_key(builder, module, output_plan, function, plan, field.key())?;

            Ok(ClassElement::new_property_definition(
                SPAN,
                PropertyDefinitionType::PropertyDefinition,
                ArenaVec::new_in(builder),
                key,
                None::<ArenaBox<'ast, TSTypeAnnotation<'ast>>>,
                field
                    .initializer()
                    .map(|initializer| {
                        emit_deferred_class_expression(builder, module, output_plan, initializer)
                    })
                    .transpose()?,
                computed,
                field.placement() == ClassFieldPlacement::Static,
                false,
                false,
                false,
                false,
                false,
                None,
                builder,
            ))
        }

        IrClassElement::StaticBlock(block) => {
            let block_ir =
                module
                    .function(block.body())
                    .ok_or(JsCodegenError::UnknownFunction {
                        function: block.body(),
                    })?;

            if block_ir.kind() != FunctionKind::ClassStaticBlock
                || block_ir.mode() != FunctionMode::Normal
                || !block_ir.parameters().is_empty()
                || block_ir.self_binding().is_some()
            {
                return Err(JsCodegenError::InvalidFunctionKind {
                    function: block.body(),
                });
            }

            Ok(ClassElement::new_static_block(
                SPAN,
                emit_function_body(builder, module, output_plan, block.body())?,
                builder,
            ))
        }
    }
}

fn emit_deferred_class_expression<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    output_plan: &JsModulePlan,
    function_id: FunctionId,
) -> Result<Expression<'ast>, JsCodegenError> {
    let function = module
        .function(function_id)
        .ok_or(JsCodegenError::UnknownFunction {
            function: function_id,
        })?;

    if function.kind() != FunctionKind::ClassFieldInitializer {
        return Err(JsCodegenError::InvalidFunctionKind {
            function: function_id,
        });
    }

    let mut statements = emit_function_body(builder, module, output_plan, function_id)?;
    let Some(Statement::ReturnStatement(return_statement)) = statements.pop() else {
        return Err(JsCodegenError::InvalidFunctionKind {
            function: function_id,
        });
    };
    let Some(expression) = return_statement.unbox().argument else {
        return Err(JsCodegenError::InvalidFunctionKind {
            function: function_id,
        });
    };

    if statements.is_empty() {
        return Ok(expression);
    }

    statements.push(Statement::new_return_statement(
        SPAN,
        Some(expression),
        builder,
    ));
    let parameters = FormalParameters::boxed(
        SPAN,
        FormalParameterKind::ArrowFormalParameters,
        ArenaVec::new_in(builder),
        None::<ArenaBox<'ast, FormalParameterRest<'ast>>>,
        builder,
    );
    let body = FunctionBody::boxed(SPAN, ArenaVec::new_in(builder), statements, builder);
    let arrow = Expression::new_arrow_function_expression(
        SPAN,
        false,
        false,
        None::<ArenaBox<'ast, TSTypeParameterDeclaration<'ast>>>,
        parameters,
        None::<ArenaBox<'ast, TSTypeAnnotation<'ast>>>,
        body,
        builder,
    );

    Ok(Expression::new_call_expression(
        SPAN,
        arrow,
        None::<ArenaBox<'ast, TSTypeParameterInstantiation<'ast>>>,
        ArenaVec::new_in(builder),
        false,
        builder,
    ))
}

fn emit_class_key<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    output_plan: &JsModulePlan,
    function: &JsFunctionIr,
    plan: &JsFunctionPlan,
    key: &ClassElementKey,
) -> Result<(AstPropertyKey<'ast>, bool), JsCodegenError> {
    match key {
        ClassElementKey::Static(name) => Ok((emit_static_object_key(builder, name), false)),
        ClassElementKey::Computed(region) => Ok((
            AstPropertyKey::from(emit_expression_region(
                builder,
                module,
                output_plan,
                function,
                plan,
                *region,
            )?),
            true,
        )),
        ClassElementKey::Private(private_name) => {
            let private_name =
                module
                    .private_name(*private_name)
                    .ok_or(JsCodegenError::UnknownPrivateName {
                        private_name: *private_name,
                    })?;

            Ok((
                AstPropertyKey::new_private_identifier(
                    SPAN,
                    builder.allocator().alloc_str(private_name.name()),
                    builder,
                ),
                false,
            ))
        }
    }
}
