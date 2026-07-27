//! JavaScript unary operations.

use super::OperationEffects;

/// A value-based ECMAScript unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOperator {
    /// Converts the operand to a number (`+`).
    Plus,

    /// Converts the operand to a number and negates it (`-`).
    Negate,

    /// Converts the operand to a 32-bit integer and inverts its bits (`~`).
    BitwiseNot,

    /// Converts the operand to boolean and negates it (`!`).
    LogicalNot,

    /// Evaluates the operand and produces `undefined` (`void`).
    Void,
}

/// Applies a unary operator to one value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnaryOp {
    operator: UnaryOperator,
}

impl UnaryOp {
    /// Creates a unary operation.
    pub const fn new(operator: UnaryOperator) -> Self {
        Self { operator }
    }

    /// Returns the applied operator.
    pub const fn operator(&self) -> UnaryOperator {
        self.operator
    }

    /// Returns the observable effects of applying this operator.
    pub const fn effects(&self) -> OperationEffects {
        match self.operator {
            UnaryOperator::Plus | UnaryOperator::Negate | UnaryOperator::BitwiseNot => {
                OperationEffects::MAY_THROW
            }

            UnaryOperator::LogicalNot | UnaryOperator::Void => OperationEffects::NONE,
        }
    }

    pub(crate) const fn operand_count(&self) -> usize {
        1
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

/// The input form of a JavaScript `typeof` operation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeofTarget {
    /// An already-evaluated value.
    Value,

    /// A runtime global name that may not exist.
    Global(Box<str>),
}

/// Applies JavaScript `typeof` semantics.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeofOp {
    target: TypeofTarget,
}

impl TypeofOp {
    /// Creates a `typeof` operation over an already-evaluated value.
    pub const fn value() -> Self {
        Self {
            target: TypeofTarget::Value,
        }
    }

    /// Creates a non-throwing `typeof` operation over a runtime global name.
    pub fn global(name: impl Into<Box<str>>) -> Self {
        Self {
            target: TypeofTarget::Global(name.into()),
        }
    }

    /// Returns the semantic input form.
    pub const fn target(&self) -> &TypeofTarget {
        &self.target
    }

    pub(crate) const fn operand_count(&self) -> usize {
        match self.target {
            TypeofTarget::Value => 1,
            TypeofTarget::Global(_) => 0,
        }
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::{UnaryOp, UnaryOperator};

    #[test]
    fn classifies_unary_throw_behavior() {
        assert!(UnaryOp::new(UnaryOperator::Plus).effects().may_throw());
        assert!(UnaryOp::new(UnaryOperator::Negate).effects().may_throw());
        assert!(
            UnaryOp::new(UnaryOperator::BitwiseNot)
                .effects()
                .may_throw()
        );

        assert!(
            !UnaryOp::new(UnaryOperator::LogicalNot)
                .effects()
                .may_throw()
        );
        assert!(!UnaryOp::new(UnaryOperator::Void).effects().may_throw());
    }
}
