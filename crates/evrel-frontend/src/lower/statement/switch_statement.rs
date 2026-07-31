//! JavaScript `switch` statement lowering.

use evrel_js_ir::{BlockTarget, JumpOp, OperationKind, SwitchCase as IrSwitchCase, SwitchOp};
use oxc_ast::ast::SwitchStatement;

use crate::{
    FrontendError,
    lower::{
        FunctionLowerer,
        declaration::{declare_scope_bindings, instantiate_switch_scope},
        expression::lower_expression,
        statement::lower_statement_list,
    },
};

/// Lowers a JavaScript switch while retaining lazy selector evaluation.
pub(super) fn lower_switch_statement(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statement: &SwitchStatement<'_>,
    labels: Box<[Box<str>]>,
) -> Result<(), FrontendError> {
    if let Some(scope) = statement.scope_id.get() {
        declare_scope_bindings(lowerer, scope)?;
    }

    // The discriminant executes before the switch's lexical environment.
    let discriminant = lower_expression(lowerer, &statement.discriminant)?;

    // Hoistable lexical declarations are initialized after the discriminant
    // and before the first case selector executes.
    instantiate_switch_scope(lowerer, statement)?;

    let completion_block = lowerer.create_block();
    let case_blocks = statement
        .cases
        .iter()
        .map(|_| lowerer.create_block())
        .collect::<Vec<_>>();
    let mut cases = Vec::with_capacity(statement.cases.len());

    for (index, case) in statement.cases.iter().enumerate() {
        let test_region = match &case.test {
            Some(test) => {
                Some(lowerer.build_expression_region(|lowerer| lower_expression(lowerer, test))?)
            }
            None => None,
        };

        cases.push(IrSwitchCase::new(
            test_region,
            BlockTarget::new(case_blocks[index], 0),
        ));
    }

    let has_default = cases.iter().any(IrSwitchCase::is_default);
    let no_match_target = (!has_default).then_some(BlockTarget::new(completion_block, 0));

    lowerer.terminate(
        OperationKind::Switch(SwitchOp::new(
            cases,
            no_match_target,
            completion_block,
            labels.clone(),
        )),
        [discriminant],
    );

    lowerer.with_switch_control(
        labels,
        completion_block,
        |lowerer| -> Result<(), FrontendError> {
            for (index, case) in statement.cases.iter().enumerate() {
                lowerer.switch_to_block(case_blocks[index]);
                lower_statement_list(lowerer, &case.consequent)?;

                if !lowerer.current_block_is_terminated() {
                    let fallthrough = case_blocks
                        .get(index + 1)
                        .copied()
                        .unwrap_or(completion_block);

                    lowerer.terminate(
                        OperationKind::Jump(JumpOp::new(BlockTarget::new(fallthrough, 0))),
                        [],
                    );
                }
            }

            Ok(())
        },
    )?;

    lowerer.switch_to_block(completion_block);

    Ok(())
}
