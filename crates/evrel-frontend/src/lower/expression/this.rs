//! JavaScript `this` expression lowering.

use evrel_js_ir::{LoadThisOp, OperationKind, ValueId};
use oxc_ast::ast::ThisExpression;

use crate::lower::FunctionLowerer;

/// Lowers a read of the current JavaScript receiver.
pub(super) fn lower_this_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    _expression: &ThisExpression,
) -> ValueId {
    lowerer.emit_value(OperationKind::LoadThis(LoadThisOp::new()), [])
}
