//! Inputs accepted by compiler entrypoints.

/// One source file submitted for compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompileInput<'source> {
    source_name: &'source str,
    source_text: &'source str,
}

impl<'source> CompileInput<'source> {
    /// Creates a compiler input.
    pub const fn new(source_name: &'source str, source_text: &'source str) -> Self {
        Self {
            source_name,
            source_text,
        }
    }

    /// Returns the source's diagnostic name or path.
    pub const fn source_name(&self) -> &'source str {
        self.source_name
    }

    /// Returns the source text.
    pub const fn source_text(&self) -> &'source str {
        self.source_text
    }
}
