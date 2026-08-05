//! Return statement lowering.

use evrel_js_ir::{ConstantOp, ConstantValue, OperationKind};
use oxc_ast::ast::ReturnStatement;

use crate::{
    FrontendError,
    lower::{FunctionLowerer, expression::lower_expression},
};

/// Lowers a function return.
pub(super) fn lower_return_statement(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statement: &ReturnStatement<'_>,
) -> Result<(), FrontendError> {
    let value = match &statement.argument {
        Some(argument) => lower_expression(lowerer, argument)?,

        None => lowerer.emit_value(
            OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
            [],
        ),
    };

    lowerer.terminate_return(value);

    Ok(())
}
