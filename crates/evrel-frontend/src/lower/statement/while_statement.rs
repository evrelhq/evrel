//! JavaScript `while` statement lowering.

use evrel_js_ir::{BlockTarget, JumpOp, OperationKind, WhileOp};
use oxc_ast::ast::WhileStatement;

use crate::{
    FrontendError,
    lower::{FunctionLowerer, expression::lower_expression, statement::lower_statement},
};

/// Lowers `while` to explicit test, body, and exit blocks owned by [`WhileOp`].
pub(super) fn lower_while_statement(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statement: &WhileStatement<'_>,
    labels: Box<[Box<str>]>,
) -> Result<(), FrontendError> {
    let test_block = lowerer.create_block();
    let body_block = lowerer.create_block();
    let exit_block = lowerer.create_block();

    lowerer.terminate(
        OperationKind::Jump(JumpOp::new(BlockTarget::new(test_block, 0))),
        [],
    );

    lowerer.switch_to_block(test_block);
    let condition = lower_expression(lowerer, &statement.test)?;

    lowerer.terminate(
        OperationKind::While(WhileOp::new(
            test_block,
            BlockTarget::new(body_block, 0),
            BlockTarget::new(exit_block, 0),
            labels.clone(),
        )),
        [condition],
    );

    lowerer.switch_to_block(body_block);

    lowerer.with_loop_control(labels, exit_block, test_block, |lowerer| {
        lower_statement(lowerer, &statement.body)
    })?;

    if !lowerer.current_block_is_terminated() {
        lowerer.terminate(
            OperationKind::Jump(JumpOp::new(BlockTarget::new(test_block, 0))),
            [],
        );
    }

    lowerer.switch_to_block(exit_block);

    Ok(())
}
