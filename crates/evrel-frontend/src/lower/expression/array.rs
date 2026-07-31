//! JavaScript array-expression lowering.

use evrel_js_ir::{ArrayLiteralElement, ArrayLiteralOp, OperationKind, ValueId};
use oxc_ast::ast::{ArrayExpression, ArrayExpressionElement};

use crate::{FrontendError, lower::FunctionLowerer};

use super::lower_expression;

/// Lowers an array literal in source evaluation order.
pub(super) fn lower_array_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &ArrayExpression<'_>,
) -> Result<ValueId, FrontendError> {
    let mut elements = Vec::with_capacity(expression.elements.len());

    for element in &expression.elements {
        match element {
            ArrayExpressionElement::Elision(_) => {
                elements.push(ArrayLiteralElement::Elision);
            }

            ArrayExpressionElement::SpreadElement(spread) => {
                let expression = lowerer.build_expression_region(|lowerer| {
                    lower_expression(lowerer, &spread.argument)
                })?;

                elements.push(ArrayLiteralElement::Spread { expression });
            }

            element => {
                let element = element
                    .as_expression()
                    .expect("array element must be an expression");
                let expression = lowerer
                    .build_expression_region(|lowerer| lower_expression(lowerer, element))?;

                elements.push(ArrayLiteralElement::Value { expression });
            }
        }
    }

    Ok(lowerer.emit_value(
        OperationKind::ArrayLiteral(ArrayLiteralOp::new(elements)),
        [],
    ))
}
