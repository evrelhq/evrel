//! Whole-program IR storage.

use std::collections::{BTreeMap, BTreeSet};

use crate::arena::Arena;
use crate::{ModuleAttribute, ModuleId, ModuleIr};

/// Canonical host-resolved identity for one module.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModuleKey(Box<str>);

impl ModuleKey {
    /// Creates a canonical module key.
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    /// Returns the canonical module key.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One inspectable ECMAScript module owned by a program.
pub struct ProgramModule {
    key: ModuleKey,
    ir: ModuleIr,
}

impl ProgramModule {
    /// Returns the module's canonical identity.
    pub const fn key(&self) -> &ModuleKey {
        &self.key
    }

    /// Returns the module-level IR.
    pub const fn ir(&self) -> &ModuleIr {
        &self.ir
    }
}

/// How JavaScript source requests another module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModuleRequestKind {
    StaticImport,
    ReExport,
    DynamicImport,
    CommonJsRequire,
}

/// One source-level request for another JavaScript module.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleRequest {
    kind: ModuleRequestKind,
    specifier: Box<str>,
    attributes: Box<[ModuleAttribute]>,
}

impl ModuleRequest {
    /// Creates a source-level module request.
    pub fn new(
        kind: ModuleRequestKind,
        specifier: impl Into<Box<str>>,
        attributes: impl Into<Box<[ModuleAttribute]>>,
    ) -> Self {
        Self {
            kind,
            specifier: specifier.into(),
            attributes: attributes.into(),
        }
    }

    /// Returns how the source requested the module.
    pub const fn kind(&self) -> ModuleRequestKind {
        self.kind
    }

    /// Returns the source-text module specifier.
    pub fn specifier(&self) -> &str {
        &self.specifier
    }

    /// Returns import attributes in source order.
    pub fn attributes(&self) -> &[ModuleAttribute] {
        &self.attributes
    }
}

/// The resolved target of an IR module dependency.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModuleTarget {
    /// A module whose IR is owned by this program.
    Internal(ModuleId),

    /// A host-managed module that Evrel cannot inspect.
    Opaque(ModuleKey),

    /// A module intentionally excluded from the generated program.
    External(ModuleKey),
}

/// A resolved dependency edge in the linked IR program.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleDependency {
    importer: ModuleId,
    request: ModuleRequest,
    target: ModuleTarget,
}

impl ModuleDependency {
    /// Creates a resolved module dependency.
    pub fn new(importer: ModuleId, request: ModuleRequest, target: ModuleTarget) -> Self {
        Self {
            importer,
            request,
            target,
        }
    }

    /// Returns the module containing the request.
    pub const fn importer(&self) -> ModuleId {
        self.importer
    }

    /// Returns the source-level module request.
    pub const fn request(&self) -> &ModuleRequest {
        &self.request
    }

    /// Returns the host-resolved target.
    pub const fn target(&self) -> &ModuleTarget {
        &self.target
    }
}

/// Owns the inspectable modules and resolved dependencies for one program.
pub struct ProgramIr {
    modules: Arena<ModuleId, ProgramModule>,
    modules_by_key: BTreeMap<ModuleKey, ModuleId>,
    entry_modules: BTreeSet<ModuleId>,
    dependencies: Vec<ModuleDependency>,
}

impl ProgramIr {
    /// Creates an empty program.
    pub fn new() -> Self {
        Self {
            modules: Arena::new(),
            modules_by_key: BTreeMap::new(),
            entry_modules: BTreeSet::new(),
            dependencies: Vec::new(),
        }
    }

    /// Adds an inspectable module and returns its program-local ID.
    pub fn add_module(&mut self, key: ModuleKey, ir: ModuleIr) -> ModuleId {
        assert!(
            !self.modules_by_key.contains_key(&key),
            "a program cannot contain the same module key twice"
        );

        let module = self.modules.alloc(ProgramModule {
            key: key.clone(),
            ir,
        });
        self.modules_by_key.insert(key, module);

        module
    }

    /// Marks an owned module as a program entry module.
    pub fn add_entry_module(&mut self, module: ModuleId) {
        assert!(
            self.modules.get(module).is_some(),
            "an entry module must be owned by the program"
        );

        self.entry_modules.insert(module);
    }

    /// Adds a resolved dependency.
    pub fn add_dependency(&mut self, dependency: ModuleDependency) {
        assert!(
            self.modules.get(dependency.importer()).is_some(),
            "a dependency importer must be owned by the program"
        );

        if let ModuleTarget::Internal(target) = dependency.target() {
            assert!(
                self.modules.get(*target).is_some(),
                "an internal dependency target must be owned by the program"
            );
        }

        self.dependencies.push(dependency);
    }

    /// Returns a program module by ID.
    pub fn module(&self, id: ModuleId) -> Option<&ProgramModule> {
        self.modules.get(id)
    }

    /// Returns mutable module-level IR by program-local module ID.
    pub fn module_ir_mut(&mut self, id: ModuleId) -> Option<&mut ModuleIr> {
        Some(&mut self.modules.get_mut(id)?.ir)
    }

    /// Returns the module ID for a canonical key.
    pub fn module_by_key(&self, key: &ModuleKey) -> Option<ModuleId> {
        self.modules_by_key.get(key).copied()
    }

    /// Iterates over owned modules in allocation order.
    pub fn modules(&self) -> impl Iterator<Item = (ModuleId, &ProgramModule)> + '_ {
        self.modules.iter()
    }

    /// Iterates over entry modules in ID order.
    pub fn entry_modules(&self) -> impl Iterator<Item = ModuleId> + '_ {
        self.entry_modules.iter().copied()
    }

    /// Returns resolved dependencies in insertion order.
    pub fn dependencies(&self) -> &[ModuleDependency] {
        &self.dependencies
    }
}

impl Default for ProgramIr {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ModuleDependency, ModuleKey, ModuleRequest, ModuleRequestKind, ModuleTarget, ProgramIr,
    };
    use crate::ModuleIr;

    #[test]
    fn owns_modules_entry_modules_and_dependencies() {
        let mut program = ProgramIr::new();
        let entry_key = ModuleKey::new("file:///entry.js");
        let dependency_key = ModuleKey::new("file:///dependency.js");
        let entry = program.add_module(entry_key.clone(), ModuleIr::new());
        let dependency = program.add_module(dependency_key, ModuleIr::new());

        program.add_entry_module(entry);
        program.add_dependency(ModuleDependency::new(
            entry,
            ModuleRequest::new(ModuleRequestKind::StaticImport, "./dependency.js", []),
            ModuleTarget::Internal(dependency),
        ));

        assert_eq!(program.module_by_key(&entry_key), Some(entry));
        assert_eq!(program.modules().count(), 2);
        assert_eq!(program.entry_modules().collect::<Vec<_>>(), vec![entry]);
        assert_eq!(program.dependencies().len(), 1);
        assert_eq!(
            program.dependencies()[0].target(),
            &ModuleTarget::Internal(dependency)
        );
    }

    #[test]
    #[should_panic(expected = "same module key twice")]
    fn rejects_duplicate_module_keys() {
        let mut program = ProgramIr::new();
        let key = ModuleKey::new("file:///module.js");

        program.add_module(key.clone(), ModuleIr::new());
        program.add_module(key, ModuleIr::new());
    }
}
