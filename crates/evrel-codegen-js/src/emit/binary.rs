//! JavaScript binary expression emission.

use evrel_ir::{BinaryOp, BinaryOperator as IrBinaryOperator};
use oxc_ast::{
    AstBuilder,
    ast::{BinaryOperator, Expression},
};
use oxc_span::SPAN;

/// Emits one JavaScript binary expression.
pub(crate) fn emit_binary_expression<'ast>(
    builder: &AstBuilder<'ast>,
    operation: &BinaryOp,
    left: Expression<'ast>,
    right: Expression<'ast>,
) -> Expression<'ast> {
    Expression::new_binary_expression(
        SPAN,
        left,
        emit_binary_operator(operation.operator()),
        right,
        builder,
    )
}

const fn emit_binary_operator(operator: IrBinaryOperator) -> BinaryOperator {
    match operator {
        IrBinaryOperator::Add => BinaryOperator::Addition,
        IrBinaryOperator::Subtract => BinaryOperator::Subtraction,
        IrBinaryOperator::Multiply => BinaryOperator::Multiplication,
        IrBinaryOperator::Divide => BinaryOperator::Division,
        IrBinaryOperator::Remainder => BinaryOperator::Remainder,
        IrBinaryOperator::Exponentiate => BinaryOperator::Exponential,

        IrBinaryOperator::LooseEqual => BinaryOperator::Equality,
        IrBinaryOperator::LooseNotEqual => BinaryOperator::Inequality,
        IrBinaryOperator::StrictEqual => BinaryOperator::StrictEquality,
        IrBinaryOperator::StrictNotEqual => BinaryOperator::StrictInequality,

        IrBinaryOperator::LessThan => BinaryOperator::LessThan,
        IrBinaryOperator::LessThanOrEqual => BinaryOperator::LessEqualThan,
        IrBinaryOperator::GreaterThan => BinaryOperator::GreaterThan,
        IrBinaryOperator::GreaterThanOrEqual => BinaryOperator::GreaterEqualThan,
        IrBinaryOperator::In => BinaryOperator::In,
        IrBinaryOperator::InstanceOf => BinaryOperator::Instanceof,

        IrBinaryOperator::ShiftLeft => BinaryOperator::ShiftLeft,
        IrBinaryOperator::ShiftRight => BinaryOperator::ShiftRight,
        IrBinaryOperator::UnsignedShiftRight => BinaryOperator::ShiftRightZeroFill,

        IrBinaryOperator::BitwiseOr => BinaryOperator::BitwiseOR,
        IrBinaryOperator::BitwiseXor => BinaryOperator::BitwiseXOR,
        IrBinaryOperator::BitwiseAnd => BinaryOperator::BitwiseAnd,
    }
}
