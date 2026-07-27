//! JavaScript regular-expression literals.

/// Creates a fresh JavaScript `RegExp` object.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RegExpLiteralOp {
    pattern: Box<str>,
    flags: Box<str>,
}

impl RegExpLiteralOp {
    /// Creates a regular-expression literal operation.
    pub fn new(pattern: impl Into<Box<str>>, flags: impl Into<Box<str>>) -> Self {
        Self {
            pattern: pattern.into(),
            flags: flags.into(),
        }
    }

    /// Returns the pattern between the literal's slashes.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Returns the canonical flag sequence.
    pub fn flags(&self) -> &str {
        &self.flags
    }

    pub(crate) const fn operand_count(&self) -> usize {
        0
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}
