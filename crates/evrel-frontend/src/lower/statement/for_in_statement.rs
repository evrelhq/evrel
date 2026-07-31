//! JavaScript `for...in` statement lowering.

use evrel_js_ir::{BlockTarget, ForInOp, JumpOp, OperationKind};
use oxc_ast::ast::ForInStatement;

use crate::{
    FrontendError,
    lower::{
        FunctionLowerer, declaration::declare_scope_bindings, expression::lower_expression,
        statement::lower_statement,
    },
};

use super::for_iteration::lower_for_iteration_left;

/// Lowers a `for...in` loop into a structured enumeration header and flat body
/// control flow.
pub(super) fn lower_for_in_statement(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statement: &ForInStatement<'_>,
    labels: Box<[Box<str>]>,
) -> Result<(), FrontendError> {
    if let Some(scope) = statement.scope_id.get() {
        declare_scope_bindings(lowerer, scope)?;
    }

    // The enumerable expression is evaluated exactly once, before enumeration.
    let object = lower_expression(lowerer, &statement.right)?;

    let header_block = lowerer.create_block();
    let body_block = lowerer.create_block();
    let exit_block = lowerer.create_block();
    let property_key = lowerer.append_produced_block_parameter(body_block);

    lowerer.terminate(
        OperationKind::Jump(JumpOp::new(BlockTarget::new(header_block, 0))),
        [],
    );

    lowerer.switch_to_block(body_block);
    let per_iteration_bindings = lower_for_iteration_left(lowerer, &statement.left, property_key)?;

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
        OperationKind::ForIn(ForInOp::new(
            BlockTarget::new(body_block, 0),
            BlockTarget::new(exit_block, 0),
            per_iteration_bindings,
            labels,
        )),
        [object],
    );

    lowerer.switch_to_block(exit_block);

    Ok(())
}
