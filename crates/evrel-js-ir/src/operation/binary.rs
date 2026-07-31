//! JavaScript binary operations.

use super::OperationEffects;

/// An ECMAScript binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOperator {
    /// Addition or string concatenation (`+`).
    Add,

    /// Numeric subtraction (`-`).
    Subtract,

    /// Numeric multiplication (`*`).
    Multiply,

    /// Numeric division (`/`).
    Divide,

    /// Numeric remainder (`%`).
    Remainder,

    /// Numeric exponentiation (`**`).
    Exponentiate,

    /// Loose equality (`==`).
    LooseEqual,

    /// Loose inequality (`!=`).
    LooseNotEqual,

    /// Strict equality (`===`).
    StrictEqual,

    /// Strict inequality (`!==`).
    StrictNotEqual,

    /// Less-than comparison (`<`).
    LessThan,

    /// Less-than-or-equal comparison (`<=`).
    LessThanOrEqual,

    /// Greater-than comparison (`>`).
    GreaterThan,

    /// Greater-than-or-equal comparison (`>=`).
    GreaterThanOrEqual,

    /// Property membership (`in`).
    In,

    /// Prototype-chain membership (`instanceof`).
    InstanceOf,

    /// Signed left shift (`<<`).
    ShiftLeft,

    /// Signed right shift (`>>`).
    ShiftRight,

    /// Unsigned right shift (`>>>`).
    UnsignedShiftRight,

    /// Bitwise OR (`|`).
    BitwiseOr,

    /// Bitwise XOR (`^`).
    BitwiseXor,

    /// Bitwise AND (`&`).
    BitwiseAnd,
}

/// Applies a binary operator to two operand values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BinaryOp {
    operator: BinaryOperator,
}

impl BinaryOp {
    /// Creates a binary operation.
    pub const fn new(operator: BinaryOperator) -> Self {
        Self { operator }
    }

    /// Returns the applied operator.
    pub const fn operator(&self) -> BinaryOperator {
        self.operator
    }

    /// Returns the observable effects of applying this operator.
    pub const fn effects(&self) -> OperationEffects {
        match self.operator {
            BinaryOperator::StrictEqual | BinaryOperator::StrictNotEqual => OperationEffects::NONE,
            _ => OperationEffects::MAY_THROW,
        }
    }

    pub(crate) const fn operand_count(&self) -> usize {
        2
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::{BinaryOp, BinaryOperator};

    #[test]
    fn classifies_binary_throw_behavior() {
        assert!(
            !BinaryOp::new(BinaryOperator::StrictEqual)
                .effects()
                .may_throw()
        );
        assert!(
            !BinaryOp::new(BinaryOperator::StrictNotEqual)
                .effects()
                .may_throw()
        );

        assert!(BinaryOp::new(BinaryOperator::Add).effects().may_throw());
        assert!(BinaryOp::new(BinaryOperator::In).effects().may_throw());
        assert!(
            BinaryOp::new(BinaryOperator::InstanceOf)
                .effects()
                .may_throw()
        );
    }
}
