//! JavaScript new-expression lowering.

use evrel_js_ir::{ConstructOp, OperationKind, ValueId};
use oxc_ast::ast::NewExpression;

use crate::{FrontendError, lower::FunctionLowerer};

use super::{arguments::lower_arguments, lower_expression};

/// Lowers an ECMAScript constructor invocation.
pub(super) fn lower_new_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &NewExpression<'_>,
) -> Result<ValueId, FrontendError> {
    let constructor = lower_expression(lowerer, &expression.callee)?;
    let arguments = lower_arguments(lowerer, &expression.arguments)?;

    Ok(lowerer.emit_value(
        OperationKind::Construct(ConstructOp::new(arguments).with_pure_annotation(expression.pure)),
        [constructor],
    ))
}
