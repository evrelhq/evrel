//! Construction API for module IR.

use crate::{
    BindingId, BindingKind, FunctionBuilder, FunctionId, FunctionKind, FunctionMode,
    FunctionProperties, JsModuleIr, LocationId, ModuleExport, ModuleImport, PrivateNameId,
    SourceFileId, SyntheticReason, TemplateSiteId, TextRange,
};

/// Builds module-owned bindings and functions.
pub struct ModuleBuilder<'ir> {
    module: &'ir mut JsModuleIr,
}

impl<'ir> ModuleBuilder<'ir> {
    /// Creates a builder for a module.
    pub fn new(module: &'ir mut JsModuleIr) -> Self {
        Self { module }
    }

    /// Returns the module's entry function.
    pub const fn entry_function(&self) -> FunctionId {
        self.module.entry_function()
    }

    /// Registers one source file contributing to this module.
    pub fn add_source_file(
        &mut self,
        name: impl Into<Box<str>>,
        text: impl Into<Box<str>>,
    ) -> SourceFileId {
        self.module.add_source_file(name, text)
    }

    /// Returns the canonical location for a source range.
    pub fn source_location(&mut self, file: SourceFileId, range: TextRange) -> LocationId {
        self.module.source_location(file, range)
    }

    /// Returns a canonical location for compiler-created IR.
    pub fn synthetic_location(
        &mut self,
        reason: SyntheticReason,
        origins: impl IntoIterator<Item = LocationId>,
    ) -> LocationId {
        self.module.synthetic_location(reason, origins)
    }

    /// Adds a static import owned by the module.
    pub fn add_import(&mut self, import: ModuleImport) {
        self.module.add_import(import);
    }

    /// Adds a static export owned by the module.
    pub fn add_export(&mut self, export: ModuleExport) {
        self.module.add_export(export);
    }

    /// Creates a canonical JavaScript binding owned by the module.
    pub fn create_binding(
        &mut self,
        declaring_function: FunctionId,
        name: impl Into<Box<str>>,
        kind: BindingKind,
    ) -> BindingId {
        self.module.create_binding(declaring_function, name, kind)
    }

    /// Creates a canonical JavaScript private name owned by the module.
    pub fn create_private_name(&mut self, name: impl Into<Box<str>>) -> PrivateNameId {
        self.module.create_private_name(name)
    }

    /// Creates a stable identity for one tagged-template syntax site.
    pub fn create_template_site(&mut self) -> TemplateSiteId {
        self.module.create_template_site()
    }

    /// Creates an empty function owned by the module.
    pub fn create_function(
        &mut self,
        kind: FunctionKind,
        mode: FunctionMode,
        parent_function: FunctionId,
    ) -> FunctionId {
        self.module.create_function(kind, mode, parent_function)
    }

    /// Creates an empty function with immutable construction-time properties.
    pub fn create_function_with_properties(
        &mut self,
        kind: FunctionKind,
        mode: FunctionMode,
        parent_function: FunctionId,
        properties: FunctionProperties,
    ) -> FunctionId {
        self.module
            .create_function_with_properties(kind, mode, parent_function, properties)
    }

    /// Creates a builder positioned at one module-owned function.
    pub fn function_builder(&mut self, function: FunctionId) -> FunctionBuilder<'_> {
        FunctionBuilder::new(self.module, function)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        BindingKind, FunctionKind, FunctionMode, JsModuleIr, LoadBindingOp, OperationKind,
    };

    use super::ModuleBuilder;

    #[test]
    fn creates_module_owned_bindings() {
        let mut module = JsModuleIr::new();
        let entry_function = module.entry_function();

        let binding = {
            let mut builder = ModuleBuilder::new(&mut module);
            builder.create_binding(entry_function, "message", BindingKind::Const)
        };

        let binding = module.binding(binding).unwrap();

        assert_eq!(module.binding_count(), 1);
        assert_eq!(binding.declaring_function(), entry_function);
        assert_eq!(binding.name(), "message");
        assert_eq!(binding.kind(), BindingKind::Const);
    }

    #[test]
    fn creates_distinct_module_owned_private_names() {
        let mut module = JsModuleIr::new();

        let (first, second) = {
            let mut builder = ModuleBuilder::new(&mut module);
            (
                builder.create_private_name("value"),
                builder.create_private_name("value"),
            )
        };

        assert_ne!(first, second);
        assert_eq!(module.private_name_count(), 2);
        assert_eq!(module.private_name(first).unwrap().name(), "value");
        assert_eq!(module.private_name(second).unwrap().name(), "value");
    }

    #[test]
    fn creates_module_owned_functions() {
        let mut module = JsModuleIr::new();

        let entry_function = module.entry_function();
        let function = ModuleBuilder::new(&mut module).create_function(
            FunctionKind::Ordinary,
            FunctionMode::Async,
            entry_function,
        );

        assert_eq!(module.function_count(), 2);
        assert_eq!(
            module.function(function).unwrap().kind(),
            FunctionKind::Ordinary
        );
        assert_eq!(
            module.function(function).unwrap().mode(),
            FunctionMode::Async
        );
        assert_eq!(
            module.function(function).unwrap().parent_function(),
            Some(entry_function)
        );
    }

    #[test]
    #[should_panic(expected = "arrow functions cannot be generators")]
    fn rejects_generator_arrow_functions() {
        let mut module = JsModuleIr::new();
        let entry = module.entry_function();

        ModuleBuilder::new(&mut module).create_function(
            FunctionKind::Arrow,
            FunctionMode::Generator,
            entry,
        );
    }

    #[test]
    fn makes_one_binding_available_to_multiple_functions() {
        let mut module = JsModuleIr::new();
        let entry_function = module.entry_function();

        let nested_function = {
            let mut builder = ModuleBuilder::new(&mut module);
            let binding = builder.create_binding(entry_function, "message", BindingKind::Let);
            let nested_function =
                builder.create_function(FunctionKind::Arrow, FunctionMode::Normal, entry_function);

            for function in [entry_function, nested_function] {
                builder.function_builder(function).append_operation(
                    crate::LocationId::UNKNOWN,
                    OperationKind::LoadBinding(LoadBindingOp::new(binding)),
                    [],
                    crate::UnwindTarget::Propagate,
                );
            }

            nested_function
        };

        assert_eq!(
            module.function(entry_function).unwrap().operation_count(),
            1
        );
        assert_eq!(
            module.function(nested_function).unwrap().operation_count(),
            1
        );
    }
}
