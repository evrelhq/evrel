//! JavaScript conditional-expression lowering.

use evrel_js_ir::{BlockTarget, IfOp, JumpOp, OperationKind, ValueId};
use oxc_ast::ast::ConditionalExpression;

use crate::{FrontendError, lower::FunctionLowerer};

use super::lower_expression;

/// Lowers a JavaScript conditional expression into structured control flow.
pub(super) fn lower_conditional_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &ConditionalExpression<'_>,
) -> Result<ValueId, FrontendError> {
    let condition = lower_expression(lowerer, &expression.test)?;

    let then_block = lowerer.create_block();
    let else_block = lowerer.create_block();
    let completion_block = lowerer.create_block();
    let result = lowerer.append_forwarded_block_parameter(completion_block);

    lowerer.terminate(
        OperationKind::If(IfOp::new(
            BlockTarget::new(then_block, 0),
            BlockTarget::new(else_block, 0),
            completion_block,
        )),
        [condition],
    );

    lowerer.switch_to_block(then_block);
    let then_value = lower_expression(lowerer, &expression.consequent)?;
    lowerer.terminate(
        OperationKind::Jump(JumpOp::new(BlockTarget::new(completion_block, 1))),
        [then_value],
    );

    lowerer.switch_to_block(else_block);
    let else_value = lower_expression(lowerer, &expression.alternate)?;
    lowerer.terminate(
        OperationKind::Jump(JumpOp::new(BlockTarget::new(completion_block, 1))),
        [else_value],
    );

    lowerer.switch_to_block(completion_block);

    Ok(result)
}
