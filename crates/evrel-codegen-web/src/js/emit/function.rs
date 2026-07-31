//! JavaScript function-body and function-object emission.

use evrel_js_ir::{CreateFunctionOp, FunctionId, FunctionKind, FunctionParameterKind, JsModuleIr};
use oxc_allocator::{Box as ArenaBox, GetAllocator, Vec as ArenaVec};
use oxc_ast::{
    AstBuilder,
    ast::{
        BindingIdentifier, BindingRestElement, Directive, Expression, FormalParameter,
        FormalParameterKind, FormalParameterRest, FormalParameters, Function, FunctionBody,
        FunctionType, Statement, TSThisParameter, TSTypeAnnotation, TSTypeParameterDeclaration,
    },
};
use oxc_span::SPAN;

use crate::{JsCodegenError, js::plan::JsModulePlan};

use super::{
    control::emit_control_body,
    pattern::{emit_binding_pattern, emit_formal_parameter_pattern},
};

/// Emits the straight-line body of one module-owned function.
pub(crate) fn emit_function_body<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    output_plan: &JsModulePlan,
    function_id: FunctionId,
) -> Result<ArenaVec<'ast, Statement<'ast>>, JsCodegenError> {
    let function = module
        .function(function_id)
        .ok_or(JsCodegenError::UnknownFunction {
            function: function_id,
        })?;

    let function_plan =
        output_plan
            .function(function_id)
            .ok_or(JsCodegenError::MissingFunctionPlan {
                function: function_id,
            })?;

    emit_control_body(builder, module, output_plan, function, function_plan)
}

/// Emits a function expression and its body.
pub(crate) fn emit_create_function_expression<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    output_plan: &JsModulePlan,
    operation_id: evrel_js_ir::OperationId,
    operation: &CreateFunctionOp,
) -> Result<Expression<'ast>, JsCodegenError> {
    let function_id = operation.function();

    let function = module
        .function(function_id)
        .ok_or(JsCodegenError::UnknownFunction {
            function: function_id,
        })?;

    let mode = function.mode();
    let function_plan =
        output_plan
            .function(function_id)
            .ok_or(JsCodegenError::MissingFunctionPlan {
                function: function_id,
            })?;
    match function.kind() {
        FunctionKind::Ordinary => {
            let name = function
                .self_binding()
                .map(|binding| {
                    let name = function_plan
                        .binding_name(binding)
                        .ok_or(JsCodegenError::UnknownBinding { binding })?;

                    Ok(BindingIdentifier::new(
                        SPAN,
                        builder.allocator().alloc_str(name),
                        builder,
                    ))
                })
                .transpose()?;

            Ok(Expression::FunctionExpression(emit_function_node(
                builder,
                module,
                output_plan,
                function_id,
                name,
            )?))
        }

        FunctionKind::Arrow => {
            if function.self_binding().is_some() {
                return Err(JsCodegenError::UnsupportedOperation {
                    operation: operation_id,
                    reason: concat!(file!(), ":", line!()),
                });
            }

            let parameters = emit_parameters(
                builder,
                module,
                output_plan,
                function_plan,
                function,
                FormalParameterKind::ArrowFormalParameters,
            )?;
            let body = FunctionBody::boxed(
                SPAN,
                emit_function_directives(builder, function),
                emit_function_body(builder, module, output_plan, function_id)?,
                builder,
            );

            Ok(Expression::new_arrow_function_expression(
                SPAN,
                false,
                mode.is_async(),
                None::<ArenaBox<'ast, TSTypeParameterDeclaration<'ast>>>,
                parameters,
                None::<ArenaBox<'ast, TSTypeAnnotation<'ast>>>,
                body,
                builder,
            ))
        }

        _ => unreachable!("the function kind was checked above"),
    }
}

pub(crate) fn emit_function_node<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    output_plan: &JsModulePlan,
    function_id: FunctionId,
    name: Option<BindingIdentifier<'ast>>,
) -> Result<ArenaBox<'ast, Function<'ast>>, JsCodegenError> {
    let function = module
        .function(function_id)
        .ok_or(JsCodegenError::UnknownFunction {
            function: function_id,
        })?;

    if matches!(
        function.kind(),
        FunctionKind::Module
            | FunctionKind::Arrow
            | FunctionKind::ClassFieldInitializer
            | FunctionKind::ClassStaticBlock
    ) {
        return Err(JsCodegenError::InvalidFunctionKind {
            function: function_id,
        });
    }

    let function_plan =
        output_plan
            .function(function_id)
            .ok_or(JsCodegenError::MissingFunctionPlan {
                function: function_id,
            })?;
    let parameters = emit_parameters(
        builder,
        module,
        output_plan,
        function_plan,
        function,
        FormalParameterKind::FormalParameter,
    )?;
    let body = FunctionBody::boxed(
        SPAN,
        emit_function_directives(builder, function),
        emit_function_body(builder, module, output_plan, function_id)?,
        builder,
    );
    let mode = function.mode();

    Ok(Function::boxed(
        SPAN,
        FunctionType::FunctionExpression,
        name,
        mode.is_generator(),
        mode.is_async(),
        false,
        None::<ArenaBox<'ast, TSTypeParameterDeclaration<'ast>>>,
        None::<ArenaBox<'ast, TSThisParameter<'ast>>>,
        parameters,
        None::<ArenaBox<'ast, TSTypeAnnotation<'ast>>>,
        Some(body),
        builder,
    ))
}

pub(crate) fn emit_function_directives<'ast>(
    builder: &AstBuilder<'ast>,
    function: &evrel_js_ir::JsFunctionIr,
) -> ArenaVec<'ast, Directive<'ast>> {
    let mut directives = ArenaVec::new_in(builder);
    if function.has_use_strict_directive() {
        directives.push(Directive::new_use_strict(builder));
    }
    directives
}

pub(crate) fn emit_function_declaration_statement<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    output_plan: &JsModulePlan,
    function_id: FunctionId,
    name: &str,
) -> Result<Statement<'ast>, JsCodegenError> {
    let identifier = BindingIdentifier::new(SPAN, builder.allocator().alloc_str(name), builder);
    let mut function =
        emit_function_node(builder, module, output_plan, function_id, Some(identifier))?;
    function.r#type = FunctionType::FunctionDeclaration;

    Ok(Statement::FunctionDeclaration(function))
}

fn emit_parameters<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    output_plan: &JsModulePlan,
    plan: &crate::js::plan::JsFunctionPlan,
    function: &evrel_js_ir::JsFunctionIr,
    kind: FormalParameterKind,
) -> Result<ArenaBox<'ast, FormalParameters<'ast>>, JsCodegenError> {
    let mut items = ArenaVec::new_in(builder);
    let mut rest = None;

    for (index, parameter) in function.parameters().iter().enumerate() {
        match parameter.kind() {
            FunctionParameterKind::Argument if rest.is_none() => {
                let (pattern, initializer) = emit_formal_parameter_pattern(
                    builder,
                    module,
                    output_plan,
                    function,
                    plan,
                    parameter.target(),
                )?;

                items.push(FormalParameter::new(
                    SPAN,
                    ArenaVec::new_in(builder),
                    pattern,
                    None::<ArenaBox<'ast, TSTypeAnnotation<'ast>>>,
                    initializer.map(|initializer| ArenaBox::new_in(initializer, builder)),
                    false,
                    None,
                    false,
                    false,
                    builder,
                ));
            }

            FunctionParameterKind::Rest
                if rest.is_none() && index + 1 == function.parameters().len() =>
            {
                let pattern = emit_binding_pattern(
                    builder,
                    module,
                    output_plan,
                    function,
                    plan,
                    parameter.target(),
                )?;

                rest = Some(FormalParameterRest::boxed(
                    SPAN,
                    ArenaVec::new_in(builder),
                    BindingRestElement::new(SPAN, pattern, builder),
                    None::<ArenaBox<'ast, TSTypeAnnotation<'ast>>>,
                    builder,
                ));
            }

            FunctionParameterKind::Argument | FunctionParameterKind::Rest => {
                return Err(JsCodegenError::UnsupportedValue {
                    value: parameter.value(),
                });
            }
        }
    }

    Ok(FormalParameters::boxed(SPAN, kind, items, rest, builder))
}
