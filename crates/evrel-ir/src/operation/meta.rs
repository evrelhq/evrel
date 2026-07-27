//! JavaScript meta-property operations.

/// A JavaScript execution-context meta-property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetaPropertyKind {
    /// The current module's metadata object.
    ImportMeta,

    /// The constructor that initiated the current construction.
    NewTarget,
}

/// Reads a JavaScript execution-context meta-property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MetaPropertyOp {
    kind: MetaPropertyKind,
}

impl MetaPropertyOp {
    /// Creates a meta-property operation.
    pub const fn new(kind: MetaPropertyKind) -> Self {
        Self { kind }
    }

    /// Returns the selected meta-property.
    pub const fn kind(&self) -> MetaPropertyKind {
        self.kind
    }

    pub(crate) const fn operand_count(&self) -> usize {
        0
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}
