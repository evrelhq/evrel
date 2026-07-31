//! JavaScript template-literal lowering.

use evrel_js_ir::{
    JsString, OperationKind, TaggedTemplateOp, TemplateLiteralOp, TemplateQuasi, ValueId,
};
use oxc_ast::ast::{TaggedTemplateExpression, TemplateLiteral};

use crate::{FrontendError, lower::FunctionLowerer};

use super::{call, lower_expression};

/// Lowers an untagged template literal in source evaluation order.
pub(super) fn lower_template_literal(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    literal: &TemplateLiteral<'_>,
) -> Result<ValueId, FrontendError> {
    let quasis = literal
        .quasis
        .iter()
        .map(|quasi| {
            let cooked = quasi
                .value
                .cooked
                .as_ref()
                .expect("untagged templates must have valid escapes");

            TemplateQuasi::new(
                quasi.value.raw.as_str(),
                Some(JsString::new(cooked.as_str(), quasi.lone_surrogates)),
            )
        })
        .collect::<Vec<_>>();
    let mut substitutions = Vec::with_capacity(literal.expressions.len());

    for expression in &literal.expressions {
        substitutions.push(
            lowerer.build_expression_region(|lowerer| lower_expression(lowerer, expression))?,
        );
    }

    Ok(lowerer.emit_value(
        OperationKind::TemplateLiteral(TemplateLiteralOp::new(quasis, substitutions)),
        [],
    ))
}

/// Lowers a tagged template while preserving its call target and site identity.
pub(super) fn lower_tagged_template_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &TaggedTemplateExpression<'_>,
) -> Result<ValueId, FrontendError> {
    let target = call::lower_call_target(lowerer, &expression.tag)?;
    let site = lowerer.create_template_site();
    let quasis = expression
        .quasi
        .quasis
        .iter()
        .map(|quasi| {
            TemplateQuasi::new(
                quasi.value.raw.as_str(),
                quasi
                    .value
                    .cooked
                    .as_ref()
                    .map(|cooked| JsString::new(cooked.as_str(), quasi.lone_surrogates)),
            )
        })
        .collect::<Vec<_>>();
    let mut substitutions = Vec::with_capacity(expression.quasi.expressions.len());

    for substitution in &expression.quasi.expressions {
        substitutions.push(
            lowerer.build_expression_region(|lowerer| lower_expression(lowerer, substitution))?,
        );
    }

    Ok(lowerer.emit_value(
        OperationKind::TaggedTemplate(TaggedTemplateOp::new(
            site,
            target.target,
            quasis,
            substitutions,
        )),
        target.operands,
    ))
}
