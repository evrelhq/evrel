//! JavaScript `continue` statement lowering.

use evrel_js_ir::{BlockTarget, JumpOp, OperationKind};
use oxc_ast::ast::ContinueStatement;

use crate::{FrontendError, lower::FunctionLowerer};

/// Lowers `continue` to its resolved loop continuation target.
pub(super) fn lower_continue_statement(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statement: &ContinueStatement<'_>,
) -> Result<(), FrontendError> {
    let label = statement.label.as_ref().map(|label| label.name.as_str());
    let target = lowerer
        .continue_target(label)
        .expect("Oxc semantic analysis must validate continue targets");

    lowerer.terminate(
        OperationKind::Jump(JumpOp::new(BlockTarget::new(target, 0))),
        [],
    );

    Ok(())
}
