//! JavaScript suspension operations.

/// Suspends execution until a value settles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AwaitOp;

impl AwaitOp {
    /// Creates an await operation.
    pub const fn new() -> Self {
        Self
    }

    pub(crate) const fn operand_count(&self) -> usize {
        1
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

/// The protocol used by a JavaScript yield expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YieldKind {
    /// Yields one value to the generator's caller.
    Value,

    /// Delegates to another iterator using `yield*`.
    Delegate,
}

/// Suspends a generator and produces its next resumption value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct YieldOp {
    kind: YieldKind,
}

impl YieldOp {
    /// Creates a yield operation.
    pub const fn new(kind: YieldKind) -> Self {
        Self { kind }
    }

    /// Returns whether this is an ordinary or delegated yield.
    pub const fn kind(&self) -> YieldKind {
        self.kind
    }

    pub(crate) const fn operand_count(&self) -> usize {
        1
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}
