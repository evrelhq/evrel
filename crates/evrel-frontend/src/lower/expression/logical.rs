//! JavaScript logical-expression lowering.

use evrel_ir::{BlockTarget, IfOp, IsNullishOp, JumpOp, OperationKind, ValueId};
use oxc_ast::ast::LogicalExpression;
use oxc_syntax::operator::LogicalOperator;

use crate::{FrontendError, lower::FunctionLowerer};

use super::lower_expression;

/// Lowers a short-circuiting JavaScript logical expression.
pub(super) fn lower_logical_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &LogicalExpression<'_>,
) -> Result<ValueId, FrontendError> {
    let left = lower_expression(lowerer, &expression.left)?;

    let right_block = lowerer.create_block();
    let completion_block = lowerer.create_block();
    let result = lowerer.append_forwarded_block_parameter(completion_block);

    let (condition, then_target, else_target) = match expression.operator {
        LogicalOperator::And => (
            left,
            BlockTarget::new(right_block, 0),
            BlockTarget::new(completion_block, 1),
        ),

        LogicalOperator::Or => (
            left,
            BlockTarget::new(completion_block, 1),
            BlockTarget::new(right_block, 0),
        ),

        LogicalOperator::Coalesce => {
            let is_nullish =
                lowerer.emit_value(OperationKind::IsNullish(IsNullishOp::new()), [left]);

            (
                is_nullish,
                BlockTarget::new(right_block, 0),
                BlockTarget::new(completion_block, 1),
            )
        }
    };

    // Operand zero selects the path. Operand one forwards the original left
    // value unchanged when evaluation short-circuits.
    lowerer.terminate(
        OperationKind::If(IfOp::new(then_target, else_target, completion_block)),
        [condition, left],
    );

    lowerer.switch_to_block(right_block);
    let right = lower_expression(lowerer, &expression.right)?;
    lowerer.terminate(
        OperationKind::Jump(JumpOp::new(BlockTarget::new(completion_block, 1))),
        [right],
    );

    lowerer.switch_to_block(completion_block);

    Ok(result)
}
