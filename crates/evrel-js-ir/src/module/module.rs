//! Module-level IR storage.

use std::collections::BTreeSet;

use crate::arena::Arena;
use crate::binding::BindingTable;
use crate::private_name::PrivateNameTable;
use crate::{
    BindingData, BindingId, BindingKind, CompilerLocation, FunctionId, FunctionKind, FunctionMode,
    FunctionProperties, JsFunctionIr, LocationId, ModuleExport, ModuleImport, PrivateNameData,
    PrivateNameId, SourceDatabase, SourceFile, SourceFileId, SyntheticReason, TemplateSiteId,
    TextRange,
};

/// Owns the bindings and functions for one JavaScript module.
pub struct JsModuleIr {
    entry_function: FunctionId,
    bindings: BindingTable,
    private_names: PrivateNameTable,
    template_site_count: usize,
    functions: Arena<FunctionId, JsFunctionIr>,
    imports: Vec<ModuleImport>,
    exports: Vec<ModuleExport>,
    sources: SourceDatabase,
}

impl JsModuleIr {
    /// Creates an empty module with one entry function.
    pub fn new() -> Self {
        Self::with_entry_properties(FunctionProperties::default())
    }

    /// Creates an empty module with construction-time properties for its entry
    /// execution context.
    pub fn with_entry_properties(properties: FunctionProperties) -> Self {
        let mut functions = Arena::new();
        let entry_function = functions.alloc(JsFunctionIr::new(
            FunctionKind::Module,
            FunctionMode::Normal,
            None,
            properties.resolve(FunctionKind::Module, false),
        ));

        Self {
            entry_function,
            bindings: BindingTable::new(),
            private_names: PrivateNameTable::new(),
            template_site_count: 0,
            functions,
            imports: Vec::new(),
            exports: Vec::new(),
            sources: SourceDatabase::new(),
        }
    }

    /// Returns the module's entry function.
    pub const fn entry_function(&self) -> FunctionId {
        self.entry_function
    }

    /// Returns the number of live bindings.
    pub const fn binding_count(&self) -> usize {
        self.bindings.len()
    }

    /// Returns the number of live private names.
    pub const fn private_name_count(&self) -> usize {
        self.private_names.len()
    }

    /// Returns the number of tagged-template sites.
    pub const fn template_site_count(&self) -> usize {
        self.template_site_count
    }

    /// Returns the number of live functions.
    pub const fn function_count(&self) -> usize {
        self.functions.len()
    }

    /// Returns static imports in source order.
    pub fn imports(&self) -> &[ModuleImport] {
        &self.imports
    }

    /// Returns static exports in source order.
    pub fn exports(&self) -> &[ModuleExport] {
        &self.exports
    }

    /// Returns a source file contributing to this module.
    pub fn source_file(&self, file: SourceFileId) -> Option<&SourceFile> {
        self.sources.file(file)
    }

    /// Returns a source location used by this module.
    pub fn location(&self, location: LocationId) -> Option<&CompilerLocation> {
        self.sources.location(location)
    }

    pub(crate) fn add_source_file(
        &mut self,
        name: impl Into<Box<str>>,
        text: impl Into<Box<str>>,
    ) -> SourceFileId {
        self.sources.add_file(name, text)
    }

    pub(crate) fn source_location(&mut self, file: SourceFileId, range: TextRange) -> LocationId {
        self.sources.source_location(file, range)
    }

    pub(crate) fn synthetic_location(
        &mut self,
        reason: SyntheticReason,
        origins: impl IntoIterator<Item = LocationId>,
    ) -> LocationId {
        self.sources.synthetic_location(reason, origins)
    }

    /// Returns a binding by ID.
    pub fn binding(&self, id: BindingId) -> Option<&BindingData> {
        self.bindings.get(id)
    }

    /// Returns a private name by ID.
    pub fn private_name(&self, id: PrivateNameId) -> Option<&PrivateNameData> {
        self.private_names.get(id)
    }

    /// Returns a function by ID.
    pub fn function(&self, id: FunctionId) -> Option<&JsFunctionIr> {
        self.functions.get(id)
    }

    /// Iterates over bindings in allocation order.
    pub fn bindings(&self) -> impl Iterator<Item = (BindingId, &BindingData)> + '_ {
        self.bindings.iter()
    }

    /// Iterates over private names in allocation order.
    pub fn private_names(&self) -> impl Iterator<Item = (PrivateNameId, &PrivateNameData)> + '_ {
        self.private_names.iter()
    }

    /// Iterates over functions in allocation order.
    pub fn functions(&self) -> impl Iterator<Item = (FunctionId, &JsFunctionIr)> + '_ {
        self.functions.iter()
    }

    /// Iterates mutably over functions in allocation order.
    ///
    /// Every yielded function is independently owned. Module-level bindings,
    /// imports, exports, and the function table itself cannot be changed
    /// through this iterator.
    pub fn functions_mut(&mut self) -> impl Iterator<Item = (FunctionId, &mut JsFunctionIr)> + '_ {
        self.functions.iter_mut()
    }

    pub(crate) fn create_binding(
        &mut self,
        declaring_function: FunctionId,
        name: impl Into<Box<str>>,
        kind: BindingKind,
    ) -> BindingId {
        assert!(
            self.functions.get(declaring_function).is_some(),
            "a binding must be declared by a live function"
        );

        self.bindings.create(declaring_function, name, kind)
    }

    pub(crate) fn create_private_name(&mut self, name: impl Into<Box<str>>) -> PrivateNameId {
        self.private_names.create(name)
    }

    pub(crate) fn create_template_site(&mut self) -> TemplateSiteId {
        let site = TemplateSiteId::from_index(self.template_site_count);
        self.template_site_count += 1;
        site
    }

    pub(crate) fn add_import(&mut self, import: ModuleImport) {
        if let Some(binding) = import.binding() {
            let binding = self
                .bindings
                .get(binding)
                .expect("a module import must reference a live binding");

            assert_eq!(
                binding.kind(),
                BindingKind::Import,
                "a module import must reference an import binding"
            );
        }

        self.imports.push(import);
    }

    pub(crate) fn add_export(&mut self, export: ModuleExport) {
        if let Some(binding) = export.binding() {
            let binding = self
                .binding(binding)
                .expect("a local export must reference a live binding");

            assert_eq!(
                binding.declaring_function(),
                self.entry_function,
                "a local export binding must be declared by the module entry function"
            );
        }

        self.exports.push(export);
    }

    pub(crate) fn replace_module_interface(
        &mut self,
        imports: Vec<ModuleImport>,
        exports: Vec<ModuleExport>,
        removed_bindings: impl IntoIterator<Item = BindingId>,
    ) {
        self.imports = imports;
        self.exports = exports;
        self.remove_bindings(removed_bindings);
    }

    pub(crate) fn create_function(
        &mut self,
        kind: FunctionKind,
        mode: FunctionMode,
        parent_function: FunctionId,
    ) -> FunctionId {
        self.create_function_with_properties(
            kind,
            mode,
            parent_function,
            FunctionProperties::default(),
        )
    }

    pub(crate) fn create_function_with_properties(
        &mut self,
        kind: FunctionKind,
        mode: FunctionMode,
        parent_function: FunctionId,
        properties: FunctionProperties,
    ) -> FunctionId {
        assert_ne!(
            kind,
            FunctionKind::Module,
            "a module cannot contain more than one module entry function"
        );
        assert!(
            self.functions.get(parent_function).is_some(),
            "a nested function must have a live parent function"
        );

        let parent_is_strict = self
            .functions
            .get(parent_function)
            .expect("a nested function must have a live parent function")
            .is_strict();
        self.functions.alloc(JsFunctionIr::new(
            kind,
            mode,
            Some(parent_function),
            properties.resolve(kind, parent_is_strict),
        ))
    }

    /// Returns a mutable function by ID without exposing other module state.
    pub fn function_mut(&mut self, id: FunctionId) -> Option<&mut JsFunctionIr> {
        self.functions.get_mut(id)
    }

    pub(crate) fn remove_functions(&mut self, functions: impl IntoIterator<Item = FunctionId>) {
        let removed_functions = functions.into_iter().collect::<BTreeSet<_>>();

        assert!(
            !removed_functions.contains(&self.entry_function),
            "cannot remove the module entry function"
        );

        for function in &removed_functions {
            assert!(
                self.functions.get(*function).is_some(),
                "cannot remove an unknown function"
            );
        }

        let removed_bindings = self
            .bindings
            .iter()
            .filter_map(|(binding, data)| {
                removed_functions
                    .contains(&data.declaring_function())
                    .then_some(binding)
            })
            .collect::<BTreeSet<_>>();

        for (function, function_ir) in self.functions.iter() {
            if removed_functions.contains(&function) {
                continue;
            }

            assert!(
                function_ir
                    .parent_function()
                    .is_none_or(|parent| !removed_functions.contains(&parent)),
                "a live function cannot have a removed lexical parent"
            );
            assert!(
                function_ir
                    .self_binding()
                    .is_none_or(|binding| !removed_bindings.contains(&binding)),
                "a live function cannot use a removed self binding"
            );

            for parameter in function_ir.parameters() {
                assert!(
                    parameter
                        .target()
                        .binding_ids()
                        .into_iter()
                        .all(|binding| !removed_bindings.contains(&binding)),
                    "a live function parameter cannot declare a removed binding"
                );
            }

            for (_, operation) in function_ir.operations() {
                operation.kind().visit_referenced_functions(|referenced| {
                    assert!(
                        !removed_functions.contains(&referenced),
                        "a live operation cannot reference a removed function"
                    );
                });
                operation.kind().visit_referenced_bindings(|binding| {
                    assert!(
                        !removed_bindings.contains(&binding),
                        "a live operation cannot reference a removed binding"
                    );
                });
            }
        }

        for import in &self.imports {
            assert!(
                import
                    .binding()
                    .is_none_or(|binding| !removed_bindings.contains(&binding)),
                "a live import cannot reference a removed binding"
            );
        }

        for export in &self.exports {
            assert!(
                export
                    .binding()
                    .is_none_or(|binding| !removed_bindings.contains(&binding)),
                "a live export cannot reference a removed binding"
            );
        }

        for binding in removed_bindings {
            self.bindings
                .remove(binding)
                .expect("removed binding was validated above");
        }

        for function in removed_functions {
            self.functions
                .remove(function)
                .expect("removed function was validated above");
        }
    }

    pub(crate) fn remove_bindings(&mut self, bindings: impl IntoIterator<Item = BindingId>) {
        let removed = bindings.into_iter().collect::<BTreeSet<_>>();

        for binding in &removed {
            assert!(
                self.bindings.get(*binding).is_some(),
                "cannot remove an unknown binding"
            );
        }

        for (_, function) in self.functions.iter() {
            assert!(
                function
                    .self_binding()
                    .is_none_or(|binding| !removed.contains(&binding)),
                "a live function cannot use a removed self binding"
            );

            for parameter in function.parameters() {
                assert!(
                    parameter
                        .target()
                        .binding_ids()
                        .into_iter()
                        .all(|binding| !removed.contains(&binding)),
                    "a live function parameter cannot declare a removed binding"
                );
            }

            for (_, operation) in function.operations() {
                operation.kind().visit_referenced_bindings(|binding| {
                    assert!(
                        !removed.contains(&binding),
                        "a live operation cannot reference a removed binding"
                    );
                });
            }
        }

        for import in &self.imports {
            assert!(
                import
                    .binding()
                    .is_none_or(|binding| !removed.contains(&binding)),
                "a live import cannot reference a removed binding"
            );
        }

        for export in &self.exports {
            assert!(
                export
                    .binding()
                    .is_none_or(|binding| !removed.contains(&binding)),
                "a live export cannot reference a removed binding"
            );
        }

        for binding in removed {
            self.bindings
                .remove(binding)
                .expect("removed binding was validated above");
        }
    }
}

impl Default for JsModuleIr {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::JsModuleIr;

    #[test]
    fn creates_a_module_with_one_entry_function() {
        let module = JsModuleIr::new();

        assert_eq!(module.binding_count(), 0);
        assert_eq!(module.private_name_count(), 0);
        assert_eq!(module.template_site_count(), 0);
        assert_eq!(module.function_count(), 1);
        assert_eq!(
            module.function(module.entry_function()).unwrap().kind(),
            crate::FunctionKind::Module
        );
        assert_eq!(
            module.function(module.entry_function()).unwrap().mode(),
            crate::FunctionMode::Normal
        );
        assert_eq!(
            module
                .function(module.entry_function())
                .unwrap()
                .parent_function(),
            None
        );
    }
}
