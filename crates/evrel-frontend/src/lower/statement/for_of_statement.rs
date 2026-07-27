//! JavaScript `for...of` statement lowering.

use evrel_ir::{BlockTarget, ForOfKind, ForOfOp, JumpOp, OperationKind};
use oxc_ast::ast::ForOfStatement;

use crate::{
    FrontendError,
    lower::{
        FunctionLowerer, declaration::declare_scope_bindings, expression::lower_expression,
        statement::lower_statement,
    },
};

use super::for_iteration::lower_for_iteration_left;

/// Lowers a `for...of` or `for await...of` loop into a structured iteration
/// header and flat body control flow.
pub(super) fn lower_for_of_statement(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statement: &ForOfStatement<'_>,
    labels: Box<[Box<str>]>,
) -> Result<(), FrontendError> {
    if let Some(scope) = statement.scope_id.get() {
        declare_scope_bindings(lowerer, scope)?;
    }

    // The iterable expression is evaluated exactly once, before iteration.
    let iterable = lower_expression(lowerer, &statement.right)?;
    let kind = if statement.r#await {
        ForOfKind::Asynchronous
    } else {
        ForOfKind::Synchronous
    };

    let header_block = lowerer.create_block();
    let body_block = lowerer.create_block();
    let exit_block = lowerer.create_block();
    let iteration_value = lowerer.append_produced_block_parameter(body_block);

    lowerer.terminate(
        OperationKind::Jump(JumpOp::new(BlockTarget::new(header_block, 0))),
        [],
    );

    lowerer.switch_to_block(body_block);
    let per_iteration_bindings =
        lower_for_iteration_left(lowerer, &statement.left, iteration_value)?;

    lowerer.with_loop_control(labels.clone(), exit_block, header_block, |lowerer| {
        lower_statement(lowerer, &statement.body)
    })?;

    if !lowerer.current_block_is_terminated() {
        lowerer.terminate(
            OperationKind::Jump(JumpOp::new(BlockTarget::new(header_block, 0))),
            [],
        );
    }

    lowerer.switch_to_block(header_block);
    lowerer.terminate(
        OperationKind::ForOf(ForOfOp::new(
            kind,
            BlockTarget::new(body_block, 0),
            BlockTarget::new(exit_block, 0),
            per_iteration_bindings,
            labels,
        )),
        [iterable],
    );

    lowerer.switch_to_block(exit_block);

    Ok(())
}
