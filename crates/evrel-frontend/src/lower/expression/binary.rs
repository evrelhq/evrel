//! JavaScript binary expression lowering.

use evrel_js_ir::{BinaryOp, BinaryOperator, HasPrivateNameOp, OperationKind, ValueId};
use oxc_ast::ast::{BinaryExpression, PrivateInExpression};
use oxc_syntax::operator::BinaryOperator as OxcBinaryOperator;

use crate::{FrontendError, lower::FunctionLowerer};

use super::lower_expression;

/// Lowers an ECMAScript binary expression.
pub(super) fn lower_binary_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &BinaryExpression<'_>,
) -> Result<ValueId, FrontendError> {
    let operator = lower_binary_operator(expression.operator)?;

    // JavaScript evaluates the left operand before the right operand.
    let left = lower_expression(lowerer, &expression.left)?;
    let right = lower_expression(lowerer, &expression.right)?;

    Ok(lowerer.emit_value(
        OperationKind::Binary(BinaryOp::new(operator)),
        [left, right],
    ))
}

/// Lowers an ECMAScript private-name membership expression.
pub(super) fn lower_private_in_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &PrivateInExpression<'_>,
) -> Result<ValueId, FrontendError> {
    let object = lower_expression(lowerer, &expression.right)?;
    let private_name = lowerer.private_name(expression.left.name.as_str());

    Ok(lowerer.emit_value(
        OperationKind::HasPrivateName(HasPrivateNameOp::new(private_name)),
        [object],
    ))
}

fn lower_binary_operator(operator: OxcBinaryOperator) -> Result<BinaryOperator, FrontendError> {
    match operator {
        OxcBinaryOperator::Addition => Ok(BinaryOperator::Add),
        OxcBinaryOperator::Subtraction => Ok(BinaryOperator::Subtract),
        OxcBinaryOperator::Multiplication => Ok(BinaryOperator::Multiply),
        OxcBinaryOperator::Division => Ok(BinaryOperator::Divide),
        OxcBinaryOperator::Remainder => Ok(BinaryOperator::Remainder),
        OxcBinaryOperator::Exponential => Ok(BinaryOperator::Exponentiate),
        OxcBinaryOperator::Equality => Ok(BinaryOperator::LooseEqual),
        OxcBinaryOperator::Inequality => Ok(BinaryOperator::LooseNotEqual),
        OxcBinaryOperator::StrictEquality => Ok(BinaryOperator::StrictEqual),
        OxcBinaryOperator::StrictInequality => Ok(BinaryOperator::StrictNotEqual),
        OxcBinaryOperator::LessThan => Ok(BinaryOperator::LessThan),
        OxcBinaryOperator::LessEqualThan => Ok(BinaryOperator::LessThanOrEqual),
        OxcBinaryOperator::GreaterThan => Ok(BinaryOperator::GreaterThan),
        OxcBinaryOperator::GreaterEqualThan => Ok(BinaryOperator::GreaterThanOrEqual),
        OxcBinaryOperator::In => Ok(BinaryOperator::In),
        OxcBinaryOperator::Instanceof => Ok(BinaryOperator::InstanceOf),
        OxcBinaryOperator::ShiftLeft => Ok(BinaryOperator::ShiftLeft),
        OxcBinaryOperator::ShiftRight => Ok(BinaryOperator::ShiftRight),
        OxcBinaryOperator::ShiftRightZeroFill => Ok(BinaryOperator::UnsignedShiftRight),
        OxcBinaryOperator::BitwiseOR => Ok(BinaryOperator::BitwiseOr),
        OxcBinaryOperator::BitwiseXOR => Ok(BinaryOperator::BitwiseXor),
        OxcBinaryOperator::BitwiseAnd => Ok(BinaryOperator::BitwiseAnd),
    }
}
