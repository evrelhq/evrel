//! JavaScript function classifications.

/// Describes the semantic form of a module-owned executable body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunctionKind {
    /// The synthetic function representing module execution.
    Module,

    /// An ordinary JavaScript function.
    Ordinary,

    /// An arrow function with lexical receiver semantics.
    Arrow,

    /// An object-literal method, getter, or setter body.
    ObjectMethod,

    /// A source-declared class constructor body.
    ClassConstructor,

    /// A class method, getter, or setter body.
    ClassMethod,

    /// A deferred class-field initializer body.
    ClassFieldInitializer,

    /// A non-callable class static-initialization block body.
    ClassStaticBlock,
}

impl FunctionKind {
    /// Returns whether ECMAScript requires this execution context to be
    /// strict regardless of its enclosing source.
    pub const fn is_intrinsically_strict(self) -> bool {
        matches!(
            self,
            Self::ClassConstructor
                | Self::ClassMethod
                | Self::ClassFieldInitializer
                | Self::ClassStaticBlock
        )
    }
}
