//! JavaScript `throw` statement lowering.

use evrel_js_ir::{OperationKind, ThrowOp};
use oxc_ast::ast::ThrowStatement;

use crate::{
    FrontendError,
    lower::{FunctionLowerer, expression::lower_expression},
};

/// Lowers a JavaScript `throw` statement.
pub(super) fn lower_throw_statement(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statement: &ThrowStatement<'_>,
) -> Result<(), FrontendError> {
    let value = lower_expression(lowerer, &statement.argument)?;

    lowerer.terminate(OperationKind::Throw(ThrowOp::new()), [value]);

    Ok(())
}
