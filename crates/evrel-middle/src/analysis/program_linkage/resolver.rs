//! Recursive ECMAScript export resolution.

use evrel_ir::{
    ModuleAttribute, ModuleDependency, ModuleExport, ModuleId, ModuleImport, ModuleRequestKind,
    ModuleTarget, ProgramBindingId, ProgramIr,
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::ImportedBindingTarget;

pub(super) fn resolve(program: &ProgramIr) -> FxHashMap<ProgramBindingId, ImportedBindingTarget> {
    Resolver::new(program).resolve_imports()
}

struct Resolver<'program> {
    program: &'program ProgramIr,
    dependencies: DependencyIndex<'program>,
}

impl<'program> Resolver<'program> {
    fn new(program: &'program ProgramIr) -> Self {
        Self {
            program,
            dependencies: DependencyIndex::new(program),
        }
    }

    fn resolve_imports(&self) -> FxHashMap<ProgramBindingId, ImportedBindingTarget> {
        let mut imported_bindings = FxHashMap::default();

        for (module, program_module) in self.program.modules() {
            for import in program_module.ir().imports() {
                let Some(binding) = import.binding() else {
                    continue;
                };

                let binding = ProgramBindingId::new(module, binding);
                let target = self.resolve_import(module, import);

                let previous = imported_bindings.insert(binding, target);
                assert!(
                    previous.is_none(),
                    "an import binding must have exactly one declaration"
                );
            }
        }

        imported_bindings
    }

    fn resolve_import(&self, importer: ModuleId, import: &ModuleImport) -> ImportedBindingTarget {
        let Some(target) = self.dependencies.target(
            importer,
            ModuleRequestKind::StaticImport,
            import.source(),
            import.attributes(),
        ) else {
            return ImportedBindingTarget::Unresolved;
        };

        match import {
            ModuleImport::Bare { .. } => ImportedBindingTarget::Unresolved,
            ModuleImport::Namespace { .. } => ImportedBindingTarget::Namespace(target.clone()),
            ModuleImport::Default { .. } => self.resolve_imported_name(target, "default"),
            ModuleImport::Named { imported, .. } => {
                self.resolve_imported_name(target, imported.as_str())
            }
        }
    }

    fn resolve_imported_name(&self, target: &ModuleTarget, name: &str) -> ImportedBindingTarget {
        let mut resolve_set = FxHashSet::default();

        match self.resolve_target_export(target, name, &mut resolve_set) {
            ExportResolution::Found(target) => target,
            ExportResolution::NotFound | ExportResolution::Unresolved => {
                ImportedBindingTarget::Unresolved
            }
        }
    }

    fn resolve_target_export(
        &self,
        target: &ModuleTarget,
        name: &str,
        resolve_set: &mut FxHashSet<(ModuleId, Box<str>)>,
    ) -> ExportResolution {
        match target {
            ModuleTarget::Internal(module) => self.resolve_export(*module, name, resolve_set),
            ModuleTarget::Opaque(module) => {
                ExportResolution::Found(ImportedBindingTarget::OpaqueExport {
                    module: module.clone(),
                    name: name.into(),
                })
            }
            ModuleTarget::External(module) => {
                ExportResolution::Found(ImportedBindingTarget::ExternalExport {
                    module: module.clone(),
                    name: name.into(),
                })
            }
        }
    }

    fn resolve_export(
        &self,
        module: ModuleId,
        name: &str,
        resolve_set: &mut FxHashSet<(ModuleId, Box<str>)>,
    ) -> ExportResolution {
        if !resolve_set.insert((module, name.into())) {
            return ExportResolution::NotFound;
        }

        let Some(module_ir) = self.program.module(module).map(|module| module.ir()) else {
            return ExportResolution::Unresolved;
        };

        if let Some(export) = module_ir.exports().iter().find(|export| {
            export
                .exported_name()
                .is_some_and(|exported| exported.as_str() == name)
        }) {
            return self.resolve_explicit_export(module, export, resolve_set);
        }

        if name == "default" {
            return ExportResolution::NotFound;
        }

        let mut star_resolution = None;

        for export in module_ir.exports() {
            let ModuleExport::Star {
                source, attributes, ..
            } = export
            else {
                continue;
            };

            let Some(target) =
                self.dependencies
                    .target(module, ModuleRequestKind::ReExport, source, attributes)
            else {
                return ExportResolution::Unresolved;
            };

            match self.resolve_target_export(target, name, resolve_set) {
                ExportResolution::NotFound => {}
                ExportResolution::Unresolved => return ExportResolution::Unresolved,
                ExportResolution::Found(candidate) => {
                    if let Some(existing) = &star_resolution {
                        if existing != &candidate {
                            return ExportResolution::Unresolved;
                        }
                    } else {
                        star_resolution = Some(candidate);
                    }
                }
            }
        }

        star_resolution.map_or(ExportResolution::NotFound, ExportResolution::Found)
    }

    fn resolve_explicit_export(
        &self,
        module: ModuleId,
        export: &ModuleExport,
        resolve_set: &mut FxHashSet<(ModuleId, Box<str>)>,
    ) -> ExportResolution {
        match export {
            ModuleExport::Empty { .. } => ExportResolution::NotFound,
            ModuleExport::Local { binding, .. } => ExportResolution::Found(
                ImportedBindingTarget::Binding(ProgramBindingId::new(module, *binding)),
            ),
            ModuleExport::Indirect {
                source,
                attributes,
                imported,
                ..
            } => {
                let Some(target) = self.dependencies.target(
                    module,
                    ModuleRequestKind::ReExport,
                    source,
                    attributes,
                ) else {
                    return ExportResolution::Unresolved;
                };

                self.resolve_target_export(target, imported.as_str(), resolve_set)
            }
            ModuleExport::Namespace {
                source, attributes, ..
            } => {
                let Some(target) = self.dependencies.target(
                    module,
                    ModuleRequestKind::ReExport,
                    source,
                    attributes,
                ) else {
                    return ExportResolution::Unresolved;
                };

                ExportResolution::Found(ImportedBindingTarget::Namespace(target.clone()))
            }
            ModuleExport::Star { .. } => ExportResolution::NotFound,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExportResolution {
    Found(ImportedBindingTarget),
    NotFound,
    Unresolved,
}

struct DependencyIndex<'program> {
    by_importer: FxHashMap<ModuleId, Vec<&'program ModuleDependency>>,
}

impl<'program> DependencyIndex<'program> {
    fn new(program: &'program ProgramIr) -> Self {
        let mut by_importer = FxHashMap::default();

        for dependency in program.dependencies() {
            by_importer
                .entry(dependency.importer())
                .or_insert_with(Vec::new)
                .push(dependency);
        }

        Self { by_importer }
    }

    fn target(
        &self,
        importer: ModuleId,
        kind: ModuleRequestKind,
        specifier: &str,
        attributes: &[ModuleAttribute],
    ) -> Option<&'program ModuleTarget> {
        let dependencies = self.by_importer.get(&importer)?;
        let mut target = None;

        for dependency in dependencies {
            let request = dependency.request();

            if request.kind() != kind
                || request.specifier() != specifier
                || request.attributes() != attributes
            {
                continue;
            }

            match target {
                None => target = Some(dependency.target()),
                Some(existing) if existing == dependency.target() => {}
                Some(_) => return None,
            }
        }

        target
    }
}
