//! JavaScript `break` statement lowering.

use evrel_js_ir::{BlockTarget, JumpOp, OperationKind};
use oxc_ast::ast::BreakStatement;

use crate::{FrontendError, lower::FunctionLowerer};

/// Lowers `break` to its resolved control-flow target.
pub(super) fn lower_break_statement(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statement: &BreakStatement<'_>,
) -> Result<(), FrontendError> {
    let label = statement.label.as_ref().map(|label| label.name.as_str());
    let target = lowerer
        .break_target(label)
        .expect("Oxc semantic analysis must validate break targets");

    lowerer.terminate(
        OperationKind::Jump(JumpOp::new(BlockTarget::new(target, 0))),
        [],
    );

    Ok(())
}
