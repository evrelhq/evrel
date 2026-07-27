//! JavaScript `do...while` statement lowering.

use evrel_ir::{BlockTarget, DoWhileOp, JumpOp, OperationKind};
use oxc_ast::ast::DoWhileStatement;

use crate::{
    FrontendError,
    lower::{FunctionLowerer, expression::lower_expression, statement::lower_statement},
};

/// Lowers `do...while` to explicit body, test, and exit blocks owned by [`DoWhileOp`].
pub(super) fn lower_do_while_statement(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statement: &DoWhileStatement<'_>,
    labels: Box<[Box<str>]>,
) -> Result<(), FrontendError> {
    let body_block = lowerer.create_block();
    let test_block = lowerer.create_block();
    let exit_block = lowerer.create_block();

    lowerer.terminate(
        OperationKind::Jump(JumpOp::new(BlockTarget::new(body_block, 0))),
        [],
    );

    lowerer.switch_to_block(body_block);

    lowerer.with_loop_control(labels.clone(), exit_block, test_block, |lowerer| {
        lower_statement(lowerer, &statement.body)
    })?;

    if !lowerer.current_block_is_terminated() {
        lowerer.terminate(
            OperationKind::Jump(JumpOp::new(BlockTarget::new(test_block, 0))),
            [],
        );
    }

    lowerer.switch_to_block(test_block);
    let condition = lower_expression(lowerer, &statement.test)?;

    lowerer.terminate(
        OperationKind::DoWhile(DoWhileOp::new(
            test_block,
            BlockTarget::new(body_block, 0),
            BlockTarget::new(exit_block, 0),
            labels,
        )),
        [condition],
    );

    lowerer.switch_to_block(exit_block);

    Ok(())
}
