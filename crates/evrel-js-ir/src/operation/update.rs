//! JavaScript update operations.

/// A JavaScript update operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpdateOperator {
    Increment,
    Decrement,
}

/// Applies JavaScript update semantics.
///
/// Result zero is the old value after `ToNumeric`.
/// Result one is the incremented or decremented value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UpdateOp {
    operator: UpdateOperator,
}

impl UpdateOp {
    /// Creates an update operation.
    pub const fn new(operator: UpdateOperator) -> Self {
        Self { operator }
    }

    /// Returns the applied update operator.
    pub const fn operator(&self) -> UpdateOperator {
        self.operator
    }

    pub(crate) const fn operand_count(&self) -> usize {
        1
    }

    pub(crate) const fn result_count(&self) -> usize {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::{UpdateOp, UpdateOperator};

    #[test]
    fn update_produces_old_and_new_numeric_values() {
        let operation = UpdateOp::new(UpdateOperator::Increment);

        assert_eq!(operation.operand_count(), 1);
        assert_eq!(operation.result_count(), 2);
    }
}
