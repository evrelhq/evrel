//! JavaScript labeled-statement lowering.

use evrel_js_ir::{BlockTarget, JumpOp, LabeledStatementData, OperationKind};
use oxc_ast::ast::{LabeledStatement, Statement};

use crate::{FrontendError, lower::FunctionLowerer};

use super::{
    do_while_statement, for_in_statement, for_of_statement, for_statement, lower_statement,
    switch_statement, while_statement,
};

/// Lowers one consecutive group of labels and its shared control target.
pub(super) fn lower_labeled_statement(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statement: &LabeledStatement<'_>,
) -> Result<(), FrontendError> {
    let (labels, body) = collect_labels(statement);

    match body {
        Statement::WhileStatement(loop_statement) => {
            return while_statement::lower_while_statement(lowerer, loop_statement, labels);
        }

        Statement::DoWhileStatement(loop_statement) => {
            return do_while_statement::lower_do_while_statement(lowerer, loop_statement, labels);
        }

        Statement::ForStatement(loop_statement) => {
            return for_statement::lower_for_statement(lowerer, loop_statement, labels);
        }

        Statement::ForInStatement(loop_statement) => {
            return for_in_statement::lower_for_in_statement(lowerer, loop_statement, labels);
        }

        Statement::ForOfStatement(loop_statement) => {
            return for_of_statement::lower_for_of_statement(lowerer, loop_statement, labels);
        }

        Statement::SwitchStatement(switch) => {
            return switch_statement::lower_switch_statement(lowerer, switch, labels);
        }

        _ => {}
    }

    let body_block = lowerer.create_block();
    let completion_block = lowerer.create_block();

    lowerer.terminate(
        OperationKind::Jump(JumpOp::new(BlockTarget::new(body_block, 0))),
        [],
    );

    lowerer.create_labeled_statement(LabeledStatementData::new(
        labels.clone(),
        body_block,
        completion_block,
    ));

    lowerer.switch_to_block(body_block);

    lowerer.with_labeled_statement_control(labels, completion_block, |lowerer| {
        lower_statement(lowerer, body)
    })?;

    if !lowerer.current_block_is_terminated() {
        lowerer.terminate(
            OperationKind::Jump(JumpOp::new(BlockTarget::new(completion_block, 0))),
            [],
        );
    }

    lowerer.switch_to_block(completion_block);

    Ok(())
}

fn collect_labels<'a>(statement: &'a LabeledStatement<'a>) -> (Box<[Box<str>]>, &'a Statement<'a>) {
    let mut labels = Vec::new();
    let mut statement = statement;

    loop {
        labels.push(statement.label.name.as_str().into());

        let Statement::LabeledStatement(nested) = &statement.body else {
            return (labels.into_boxed_slice(), &statement.body);
        };

        statement = nested;
    }
}
