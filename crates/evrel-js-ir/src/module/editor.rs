//! Invariant-preserving mutation of module-owned IR entities.

use crate::{BindingId, FunctionId, JsModuleIr, ModuleExport, ModuleImport};

/// Mutates an existing module while preserving cross-function invariants.
pub struct ModuleEditor<'ir> {
    ir: &'ir mut JsModuleIr,
}

impl<'ir> ModuleEditor<'ir> {
    /// Creates an editor for an existing module.
    pub fn new(ir: &'ir mut JsModuleIr) -> Self {
        Self { ir }
    }

    /// Atomically removes functions and the bindings declared by them.
    pub fn remove_functions(&mut self, functions: impl IntoIterator<Item = FunctionId>) {
        self.ir.remove_functions(functions);
    }

    /// Replaces static imports and exports while removing bindings that the
    /// replacement interface no longer references.
    pub fn replace_module_interface(
        &mut self,
        imports: Vec<ModuleImport>,
        exports: Vec<ModuleExport>,
        removed_bindings: impl IntoIterator<Item = BindingId>,
    ) {
        self.ir
            .replace_module_interface(imports, exports, removed_bindings);
    }

    /// Atomically removes bindings that are no longer referenced by the module.
    pub fn remove_bindings(&mut self, bindings: impl IntoIterator<Item = BindingId>) {
        self.ir.remove_bindings(bindings);
    }
}

#[cfg(test)]
mod tests {
    use crate::{BindingKind, FunctionKind, FunctionMode, JsModuleIr, ModuleBuilder};

    use super::ModuleEditor;

    #[test]
    fn removes_functions_and_their_bindings() {
        let mut module = JsModuleIr::new();
        let entry = module.entry_function();
        let (function, binding) = {
            let mut builder = ModuleBuilder::new(&mut module);
            let function =
                builder.create_function(FunctionKind::Ordinary, FunctionMode::Normal, entry);
            let binding = builder.create_binding(function, "local", BindingKind::Let);

            (function, binding)
        };

        ModuleEditor::new(&mut module).remove_functions([function]);

        assert!(module.function(function).is_none());
        assert!(module.binding(binding).is_none());
    }
}
