//! JavaScript `continue` statement lowering.

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

    lowerer.terminate_continue(target);

    Ok(())
}
