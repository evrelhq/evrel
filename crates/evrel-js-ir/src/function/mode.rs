//! JavaScript function execution modes.

/// Describes a function's invocation and completion protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FunctionMode {
    /// An ordinary synchronous function.
    #[default]
    Normal,

    /// A function returning a promise.
    Async,

    /// A function returning a generator object.
    Generator,

    /// A function returning an async-generator object.
    AsyncGenerator,
}

impl FunctionMode {
    /// Returns whether invocation uses async-function semantics.
    pub const fn is_async(self) -> bool {
        matches!(self, Self::Async | Self::AsyncGenerator)
    }

    /// Returns whether invocation uses generator semantics.
    pub const fn is_generator(self) -> bool {
        matches!(self, Self::Generator | Self::AsyncGenerator)
    }
}

#[cfg(test)]
mod tests {
    use super::FunctionMode;

    #[test]
    fn classifies_function_execution_modes() {
        assert!(!FunctionMode::Normal.is_async());
        assert!(!FunctionMode::Normal.is_generator());

        assert!(FunctionMode::Async.is_async());
        assert!(!FunctionMode::Async.is_generator());

        assert!(!FunctionMode::Generator.is_async());
        assert!(FunctionMode::Generator.is_generator());

        assert!(FunctionMode::AsyncGenerator.is_async());
        assert!(FunctionMode::AsyncGenerator.is_generator());
    }
}
