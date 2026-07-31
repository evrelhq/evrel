//! JavaScript function operations.

use crate::FunctionId;

/// Reads the current JavaScript receiver value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LoadThisOp;

impl LoadThisOp {
    /// Creates a receiver-load operation.
    pub const fn new() -> Self {
        Self
    }

    pub(crate) const fn operand_count(&self) -> usize {
        0
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

impl Default for LoadThisOp {
    fn default() -> Self {
        Self::new()
    }
}

/// Reads the implicit `arguments` binding of the enclosing ordinary function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LoadArgumentsOp;

impl LoadArgumentsOp {
    /// Creates an arguments-load operation.
    pub const fn new() -> Self {
        Self
    }

    pub(crate) const fn operand_count(&self) -> usize {
        0
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

impl Default for LoadArgumentsOp {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates a runtime function object for a module-owned function body.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CreateFunctionOp {
    function: FunctionId,
}

impl CreateFunctionOp {
    /// Creates a function-object operation.
    pub const fn new(function: FunctionId) -> Self {
        Self { function }
    }

    /// Returns the static function body being instantiated.
    pub const fn function(&self) -> FunctionId {
        self.function
    }

    pub(crate) const fn operand_count(&self) -> usize {
        0
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

#[cfg(test)]
mod tests {
    use crate::FunctionId;

    use super::{CreateFunctionOp, LoadArgumentsOp, LoadThisOp};

    #[test]
    fn defines_the_receiver_load_shape() {
        let operation = LoadThisOp::new();

        assert_eq!(operation.operand_count(), 0);
        assert_eq!(operation.result_count(), 1);
    }

    #[test]
    fn defines_the_arguments_load_shape() {
        let operation = LoadArgumentsOp::new();

        assert_eq!(operation.operand_count(), 0);
        assert_eq!(operation.result_count(), 1);
    }

    #[test]
    fn stores_the_function_body_identity() {
        let function = FunctionId::from_index(3);
        let operation = CreateFunctionOp::new(function);

        assert_eq!(operation.function(), function);
        assert_eq!(operation.operand_count(), 0);
        assert_eq!(operation.result_count(), 1);
    }
}
