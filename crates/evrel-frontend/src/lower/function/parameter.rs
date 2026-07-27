//! Function parameter lowering.

use evrel_ir::{BindingKind, BindingPattern, FunctionParameterKind, PatternExpression};
use oxc_ast::ast::{BindingPattern as OxcBindingPattern, Expression, FormalParameters};

use crate::{
    FrontendError,
    lower::{
        FunctionLowerer,
        expression::lower_expression,
        pattern::{declare_pattern_bindings, lower_binding_pattern},
    },
};

/// Lowers function parameters at the function boundary.
pub(crate) fn lower_function_parameters(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    parameters: &FormalParameters<'_>,
) -> Result<(), FrontendError> {
    if parameters.items.iter().any(|parameter| {
        parameter.accessibility.is_some() || parameter.readonly || parameter.r#override
    }) {
        return Err(FrontendError::UnsupportedParameterProperty);
    }

    // Parameter bindings all exist before any parameter initializer executes.
    for parameter in &parameters.items {
        declare_pattern_bindings(lowerer, &parameter.pattern, BindingKind::Parameter);
    }

    if let Some(parameter) = &parameters.rest {
        declare_pattern_bindings(lowerer, &parameter.rest.argument, BindingKind::Parameter);
    }

    for parameter in &parameters.items {
        lower_parameter_pattern(
            lowerer,
            &parameter.pattern,
            parameter.initializer.as_deref(),
            FunctionParameterKind::Argument,
        )?;
    }

    if let Some(parameter) = &parameters.rest {
        lower_parameter_pattern(
            lowerer,
            &parameter.rest.argument,
            None,
            FunctionParameterKind::Rest,
        )?;
    }

    Ok(())
}

fn lower_parameter_pattern(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    pattern: &OxcBindingPattern<'_>,
    initializer: Option<&Expression<'_>>,
    kind: FunctionParameterKind,
) -> Result<(), FrontendError> {
    let mut target = lower_binding_pattern(lowerer, pattern)?;

    if let Some(initializer) = initializer {
        let region =
            lowerer.build_expression_region(|lowerer| lower_expression(lowerer, initializer))?;
        target = BindingPattern::default(target, PatternExpression::new(region));
    }

    lowerer.append_parameter(kind, target);

    Ok(())
}
