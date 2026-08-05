//! JavaScript update-expression lowering.

use evrel_js_ir::{OperationKind, UpdateOp, UpdateOperator, ValueId};
use oxc_ast::ast::UpdateExpression;
use oxc_syntax::operator::UpdateOperator as OxcUpdateOperator;

use crate::{FrontendError, lower::FunctionLowerer};

use super::assignment::{
    load_assignment_reference, lower_simple_assignment_reference, store_assignment_reference,
};

/// Lowers a JavaScript prefix or postfix update expression.
pub(super) fn lower_update_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &UpdateExpression<'_>,
) -> Result<ValueId, FrontendError> {
    let reference = lower_simple_assignment_reference(lowerer, &expression.argument)?;
    let current = load_assignment_reference(lowerer, &reference);

    let operator = match expression.operator {
        OxcUpdateOperator::Increment => UpdateOperator::Increment,
        OxcUpdateOperator::Decrement => UpdateOperator::Decrement,
    };

    let results = lowerer.emit(OperationKind::Update(UpdateOp::new(operator)), [current]);

    let (old_numeric, new_numeric) = {
        let [old_numeric, new_numeric] = results.as_slice() else {
            unreachable!("update operations produce exactly two values");
        };

        (*old_numeric, *new_numeric)
    };

    store_assignment_reference(lowerer, reference, new_numeric);

    Ok(if expression.prefix {
        new_numeric
    } else {
        old_numeric
    })
}
