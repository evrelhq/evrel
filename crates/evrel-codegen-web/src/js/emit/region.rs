//! Planned expression-region emission.

use evrel_js_ir::{JsFunctionIr, JsModuleIr, RegionId};
use oxc_allocator::Vec as ArenaVec;
use oxc_ast::{AstBuilder, ast::Expression};
use oxc_span::SPAN;

use crate::{
    JsCodegenError,
    js::plan::{
        JsExpressionRegionContinuation, JsExpressionRegionStep, JsFunctionPlan, JsModulePlan,
    },
};

use super::{
    FunctionEmission,
    control::emit_edge_transfer_expressions,
    operand::emit_operand_expression,
    sequence::{emit_operations_as_expressions, expression_sequence},
    value::emit_value_expression,
};

pub(crate) fn emit_expression_region<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    output_plan: &JsModulePlan,
    function: &JsFunctionIr,
    function_plan: &JsFunctionPlan,
    region: RegionId,
) -> Result<Expression<'ast>, JsCodegenError> {
    let plan = function_plan
        .region(region)
        .ok_or(JsCodegenError::UnsupportedExpressionRegion { region })?;

    emit_region_step(
        FunctionEmission::new(builder, module, output_plan, function, function_plan),
        plan.root(),
    )
}

fn emit_region_step<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    step: &JsExpressionRegionStep,
) -> Result<Expression<'ast>, JsCodegenError> {
    let FunctionEmission {
        builder,
        module,
        output_plan,
        function,
        plan,
    } = emission;
    let JsExpressionRegionStep::Block {
        block,
        continuation,
    } = step
    else {
        return Ok(Expression::new_boolean_literal(SPAN, true, builder));
    };
    let block_data = function
        .block(*block)
        .ok_or(JsCodegenError::UnknownBlock { block: *block })?;
    let mut expressions = emit_operations_as_expressions(
        builder,
        module,
        output_plan,
        function,
        plan,
        block_data.operations(),
    )?;

    let continuation = match continuation {
        JsExpressionRegionContinuation::Yield(value) => {
            emit_operand_expression(builder, module, output_plan, function, plan, *value)?
        }
        JsExpressionRegionContinuation::Jump { edge, next } => {
            expressions.extend(emit_edge_transfer_expressions(
                builder, function, plan, *edge,
            )?);
            emit_region_step(emission, next)?
        }
        JsExpressionRegionContinuation::Branch {
            condition,
            then_edge,
            then_step,
            else_edge,
            else_step,
            next,
        } => {
            let consequent = emit_region_branch(emission, *then_edge, then_step)?;
            let alternate = emit_region_branch(emission, *else_edge, else_step)?;
            let conditional = Expression::new_conditional_expression(
                SPAN,
                emit_value_expression(builder, function, plan, *condition)?,
                consequent,
                alternate,
                builder,
            );
            if let Some(next) = next {
                let next = emit_region_step(emission, next)?;
                expression_sequence(
                    builder,
                    ArenaVec::from_array_in([conditional, next], builder),
                )
            } else {
                conditional
            }
        }
    };

    expressions.push(continuation);
    Ok(expression_sequence(builder, expressions))
}

fn emit_region_branch<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    edge: crate::js::plan::JsEdgeKey,
    step: &JsExpressionRegionStep,
) -> Result<Expression<'ast>, JsCodegenError> {
    let builder = emission.builder;
    let mut expressions =
        emit_edge_transfer_expressions(builder, emission.function, emission.plan, edge)?;
    expressions.push(emit_region_step(emission, step)?);

    Ok(expression_sequence(builder, expressions))
}
