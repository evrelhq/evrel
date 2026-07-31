//! Resolution of module bindings across a program.

mod resolver;

#[cfg(test)]
mod tests;

use evrel_ir::{ModuleKey, ModuleTarget, ProgramBindingId, ProgramIr};
use rustc_hash::FxHashMap;

/// The program-level target referenced by an imported local binding.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ImportedBindingTarget {
    /// A live binding owned by an inspectable module.
    Binding(ProgramBindingId),

    /// A module namespace object.
    Namespace(ModuleTarget),

    /// A named export of a host-managed module Evrel cannot inspect.
    OpaqueExport { module: ModuleKey, name: Box<str> },

    /// A named export of a module excluded from the generated program.
    ExternalExport { module: ModuleKey, name: Box<str> },

    /// The import could not be resolved uniquely.
    Unresolved,
}

/// Cross-module binding resolution for one program.
#[derive(Debug)]
pub struct ProgramLinkage {
    imported_bindings: FxHashMap<ProgramBindingId, ImportedBindingTarget>,
}

impl ProgramLinkage {
    /// Resolves imported bindings across the program.
    pub fn analyze(program: &ProgramIr) -> Self {
        Self {
            imported_bindings: resolver::resolve(program),
        }
    }

    /// Returns the target referenced by an imported binding.
    ///
    /// Returns `None` when `binding` is not an imported binding. An imported
    /// binding whose target cannot be determined returns
    /// [`ImportedBindingTarget::Unresolved`].
    pub fn imported_binding(&self, binding: ProgramBindingId) -> Option<&ImportedBindingTarget> {
        self.imported_bindings.get(&binding)
    }
}
