//! JavaScript await-expression lowering.

use evrel_ir::{AwaitOp, OperationKind, ValueId};
use oxc_ast::ast::AwaitExpression;

use crate::{FrontendError, lower::FunctionLowerer};

use super::lower_expression;

/// Lowers an await expression.
pub(super) fn lower_await_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &AwaitExpression<'_>,
) -> Result<ValueId, FrontendError> {
    let argument = lower_expression(lowerer, &expression.argument)?;

    Ok(lowerer.emit_value(OperationKind::Await(AwaitOp::new()), [argument]))
}
