//! JavaScript module operations.

/// The loading protocol selected by a dynamic import expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DynamicImportPhase {
    /// Loads and evaluates a module through ordinary `import()`.
    Evaluation,

    /// Loads a module's source representation.
    Source,

    /// Defers module evaluation.
    Defer,
}

/// Dynamically requests a JavaScript module.
///
/// Operand layout is `[specifier, options?]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DynamicImportOp {
    phase: DynamicImportPhase,
    has_options: bool,
}

impl DynamicImportOp {
    /// Creates a dynamic import operation.
    pub const fn new(phase: DynamicImportPhase, has_options: bool) -> Self {
        Self { phase, has_options }
    }

    /// Returns the selected module-loading phase.
    pub const fn phase(&self) -> DynamicImportPhase {
        self.phase
    }

    /// Returns whether the second operand supplies import options.
    pub const fn has_options(&self) -> bool {
        self.has_options
    }

    pub(crate) const fn operand_count(&self) -> usize {
        1 + self.has_options as usize
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}
