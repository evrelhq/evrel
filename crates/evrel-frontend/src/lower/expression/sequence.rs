//! JavaScript sequence-expression lowering.

use evrel_js_ir::ValueId;
use oxc_ast::ast::SequenceExpression;

use crate::{FrontendError, lower::FunctionLowerer};

use super::lower_expression;

/// Evaluates sequence elements left-to-right and returns the final value.
pub(super) fn lower_sequence_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    sequence: &SequenceExpression<'_>,
) -> Result<ValueId, FrontendError> {
    let Some((last, preceding)) = sequence.expressions.split_last() else {
        return Err(FrontendError::UnsupportedExpression);
    };

    for expression in preceding {
        lower_expression(lowerer, expression)?;
    }

    lower_expression(lowerer, last)
}
