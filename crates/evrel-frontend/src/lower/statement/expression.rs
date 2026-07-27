//! JavaScript expression-statement lowering.

use oxc_ast::ast::ExpressionStatement;

use crate::{
    FrontendError,
    lower::{FunctionLowerer, expression::lower_expression},
};

/// Lowers an expression evaluated in statement position.
pub(super) fn lower_expression_statement(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    statement: &ExpressionStatement<'_>,
) -> Result<(), FrontendError> {
    // The expression is evaluated for its runtime effects. Its resulting value
    // is intentionally discarded in statement position.
    lower_expression(lowerer, &statement.expression)?;

    Ok(())
}
