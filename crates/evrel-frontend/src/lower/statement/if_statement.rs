//! JavaScript `if` statement lowering.

use evrel_js_ir::{BlockTarget, IfOp, JumpOp, OperationKind};
use oxc_ast::ast::IfStatement;

use crate::{
    FrontendError,
    lower::{FunctionLowerer, expression::lower_expression, statement::lower_statement},
};

/// Lowers a structured JavaScript `if` statement.
pub(super) fn lower_if_statement(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statement: &IfStatement<'_>,
) -> Result<(), FrontendError> {
    let condition = lower_expression(lowerer, &statement.test)?;

    let then_block = lowerer.create_block();
    let else_block = statement.alternate.as_ref().map(|_| lowerer.create_block());
    let completion_block = lowerer.create_block();
    let else_target = else_block.unwrap_or(completion_block);

    lowerer.terminate(
        OperationKind::If(IfOp::new(
            BlockTarget::new(then_block, 0),
            BlockTarget::new(else_target, 0),
            completion_block,
        )),
        [condition],
    );

    lowerer.switch_to_block(then_block);
    lower_statement(lowerer, &statement.consequent)?;

    if !lowerer.current_block_is_terminated() {
        lowerer.terminate(
            OperationKind::Jump(JumpOp::new(BlockTarget::new(completion_block, 0))),
            [],
        );
    }

    if let (Some(alternate), Some(else_block)) = (&statement.alternate, else_block) {
        lowerer.switch_to_block(else_block);
        lower_statement(lowerer, alternate)?;

        if !lowerer.current_block_is_terminated() {
            lowerer.terminate(
                OperationKind::Jump(JumpOp::new(BlockTarget::new(completion_block, 0))),
                [],
            );
        }
    }

    lowerer.switch_to_block(completion_block);

    Ok(())
}
