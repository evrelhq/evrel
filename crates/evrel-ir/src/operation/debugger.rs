//! JavaScript debugger operations.

/// Executes an ECMAScript `debugger` statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DebuggerOp;

impl DebuggerOp {
    pub const fn new() -> Self {
        Self
    }

    pub(crate) const fn operand_count(&self) -> usize {
        0
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }
}

impl Default for DebuggerOp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::DebuggerOp;

    #[test]
    fn defines_the_debugger_operation_shape() {
        let operation = DebuggerOp::new();

        assert_eq!(operation.operand_count(), 0);
        assert_eq!(operation.result_count(), 0);
    }
}
