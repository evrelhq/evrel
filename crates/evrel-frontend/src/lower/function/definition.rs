//! JavaScript function-definition lowering.

use evrel_ir::{FunctionId, FunctionKind, FunctionMode};
use oxc_ast::ast::{Function, FunctionType};
use oxc_span::GetSpan;

use crate::{
    FrontendError,
    lower::{
        FunctionLowerer,
        declaration::{declare_scope_bindings, instantiate_function_scope},
    },
};

use super::{lower_function_body, lower_function_parameters, lower_function_properties};

/// Lowers an ordinary function definition into a nested `FunctionIr`.
pub(crate) fn lower_ordinary_function_definition(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    function: &Function<'_>,
) -> Result<FunctionId, FrontendError> {
    lower_function_definition(lowerer, function, FunctionKind::Ordinary)
}

/// Lowers an object-literal method or accessor into a nested `FunctionIr`.
pub(crate) fn lower_object_method_function(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    function: &Function<'_>,
) -> Result<FunctionId, FrontendError> {
    lower_function_definition(lowerer, function, FunctionKind::ObjectMethod)
}

/// Lowers a class method-like definition into a nested `FunctionIr`.
pub(crate) fn lower_class_element_function(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    function: &Function<'_>,
    constructor: bool,
) -> Result<FunctionId, FrontendError> {
    let kind = if constructor {
        FunctionKind::ClassConstructor
    } else {
        FunctionKind::ClassMethod
    };

    lower_function_definition(lowerer, function, kind)
}

fn lower_function_definition(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    function: &Function<'_>,
    kind: FunctionKind,
) -> Result<FunctionId, FrontendError> {
    lowerer.with_span(function.span(), |lowerer| {
        lower_function_definition_at_current_location(lowerer, function, kind)
    })
}

fn lower_function_definition_at_current_location(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    function: &Function<'_>,
    kind: FunctionKind,
) -> Result<FunctionId, FrontendError> {
    assert!(matches!(
        kind,
        FunctionKind::Ordinary
            | FunctionKind::ObjectMethod
            | FunctionKind::ClassConstructor
            | FunctionKind::ClassMethod
    ));

    let body = function
        .body
        .as_ref()
        .expect("runtime function definition must have a body");
    let scope = function
        .scope_id
        .get()
        .expect("semantic analysis must assign the function scope");

    let (function_id, result) = lowerer.build_nested_function_with_properties(
        kind,
        function_mode(function),
        lower_function_properties(body),
        |nested| {
            lower_function_parameters(nested, &function.params)?;
            declare_scope_bindings(nested, scope)?;

            if kind == FunctionKind::Ordinary
                && function.r#type == FunctionType::FunctionExpression
                && let Some(identifier) = &function.id
            {
                let binding = nested.binding_for_symbol(identifier.symbol_id());

                nested.set_self_binding(binding);
            }

            instantiate_function_scope(nested, scope, &body.statements)?;
            lower_function_body(nested, body)
        },
    );

    result?;

    Ok(function_id)
}

fn function_mode(function: &Function<'_>) -> FunctionMode {
    match (function.r#async, function.generator) {
        (false, false) => FunctionMode::Normal,
        (true, false) => FunctionMode::Async,
        (false, true) => FunctionMode::Generator,
        (true, true) => FunctionMode::AsyncGenerator,
    }
}
