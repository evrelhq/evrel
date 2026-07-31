//! JavaScript yield-expression lowering.

use evrel_js_ir::{ConstantOp, ConstantValue, OperationKind, ValueId, YieldKind, YieldOp};
use oxc_ast::ast::YieldExpression;

use crate::{FrontendError, lower::FunctionLowerer};

use super::lower_expression;

/// Lowers a yield expression.
pub(super) fn lower_yield_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &YieldExpression<'_>,
) -> Result<ValueId, FrontendError> {
    let argument = match &expression.argument {
        Some(argument) => lower_expression(lowerer, argument)?,
        None => lowerer.emit_value(
            OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
            [],
        ),
    };
    let kind = if expression.delegate {
        YieldKind::Delegate
    } else {
        YieldKind::Value
    };

    Ok(lowerer.emit_value(OperationKind::Yield(YieldOp::new(kind)), [argument]))
}
