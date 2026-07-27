//! JavaScript classical `for` statement lowering.

use evrel_ir::{BindingId, BlockTarget, ForOp, IfOp, JumpOp, OperationKind};
use oxc_ast::ast::{ForStatement, ForStatementInit, VariableDeclarationKind};

use crate::{
    FrontendError,
    lower::{
        FunctionLowerer, declaration::declare_scope_bindings, expression::lower_expression,
        statement::lower_statement,
    },
};

use super::variable::lower_variable_declaration;

/// Lowers a classical JavaScript `for` loop into a canonical loop host with
/// explicit initializer, test, body, update, and exit phases.
pub(super) fn lower_for_statement(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statement: &ForStatement<'_>,
    labels: Box<[Box<str>]>,
) -> Result<(), FrontendError> {
    if let Some(scope) = statement.scope_id.get() {
        declare_scope_bindings(lowerer, scope)?;
    }

    let initializer_block = lowerer.create_block();

    lowerer.terminate(
        OperationKind::Jump(JumpOp::new(BlockTarget::new(initializer_block, 0))),
        [],
    );

    lowerer.switch_to_block(initializer_block);

    let mut per_iteration_bindings: Box<[BindingId]> = Box::new([]);

    if let Some(initializer) = &statement.init {
        match initializer {
            ForStatementInit::VariableDeclaration(declaration) => {
                let bindings = lower_variable_declaration(lowerer, declaration)?;

                if declaration.kind == VariableDeclarationKind::Let {
                    per_iteration_bindings = bindings;
                }
            }

            initializer => {
                let expression = initializer
                    .as_expression()
                    .expect("non-declaration for initializer must be an expression");

                lower_expression(lowerer, expression)?;
            }
        }
    }

    let loop_block = lowerer.create_block();
    let test_block = lowerer.create_block();
    let body_block = lowerer.create_block();
    let update_block = lowerer.create_block();
    let exit_block = lowerer.create_block();

    lowerer.terminate(
        OperationKind::Jump(JumpOp::new(BlockTarget::new(loop_block, 0))),
        [],
    );

    let operation = ForOp::new(
        initializer_block,
        BlockTarget::new(test_block, 0),
        body_block,
        update_block,
        exit_block,
        per_iteration_bindings,
        labels.clone(),
    );
    lowerer.switch_to_block(loop_block);
    lowerer.terminate(OperationKind::For(operation), []);

    lowerer.switch_to_block(test_block);

    if let Some(test) = &statement.test {
        let condition = lower_expression(lowerer, test)?;

        lowerer.terminate(
            OperationKind::If(IfOp::new(
                BlockTarget::new(body_block, 0),
                BlockTarget::new(exit_block, 0),
                exit_block,
            )),
            [condition],
        );
    } else {
        lowerer.terminate(
            OperationKind::Jump(JumpOp::new(BlockTarget::new(body_block, 0))),
            [],
        );
    }

    lowerer.switch_to_block(body_block);

    lowerer.with_loop_control(labels, exit_block, update_block, |lowerer| {
        lower_statement(lowerer, &statement.body)
    })?;

    if !lowerer.current_block_is_terminated() {
        lowerer.terminate(
            OperationKind::Jump(JumpOp::new(BlockTarget::new(update_block, 0))),
            [],
        );
    }

    lowerer.switch_to_block(update_block);

    if let Some(update) = &statement.update {
        lower_expression(lowerer, update)?;
    }

    lowerer.terminate(
        OperationKind::Jump(JumpOp::new(BlockTarget::new(loop_block, 0))),
        [],
    );

    lowerer.switch_to_block(exit_block);

    Ok(())
}
