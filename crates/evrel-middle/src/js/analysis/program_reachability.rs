//! Joint module-evaluation and binding reachability for a resolved program.

use std::collections::VecDeque;

use evrel_js_ir::{
    ConstantValue, JsProgramIr, ModuleAttribute, ModuleDependency, ModuleExport, ModuleId,
    ModuleImport, ModuleRequestKind, ModuleTarget, OperationKind, ProgramBindingId,
    ValueDefinition,
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{ImportedBindingTarget, ProgramLinkage};
use crate::js::work_queue::WorkQueue;

/// Program entities required by module evaluation or an observable interface.
///
/// Module evaluation, binding liveness, and export liveness are separate facts
/// in one fixed point: a bare import can require evaluating a module without
/// using any of its bindings, while a used import can make bindings and exports
/// live across modules.
#[derive(Debug, Clone)]
pub struct ProgramReachability {
    evaluated_modules: FxHashSet<ModuleId>,
    live_bindings: FxHashSet<ProgramBindingId>,
    live_exports: FxHashSet<ProgramExport>,
}

impl ProgramReachability {
    /// Computes conservative reachability from program entry modules.
    ///
    /// If a reachable static import or re-export has no unique resolved edge,
    /// every owned module and binding is retained because the missing target
    /// could refer to any program entity.
    pub fn compute(program: &JsProgramIr, linkage: &ProgramLinkage) -> Self {
        let dependencies = DependencyIndex::new(program);
        let mut evaluated_modules = FxHashSet::default();
        let mut live_bindings = FxHashSet::default();
        let mut live_namespaces = FxHashSet::default();
        let mut work = WorkQueue::new();

        for module in program.entry_modules() {
            work.push(ReachabilityFact::EvaluateModule(module));

            if !retain_entry_exports(program, &dependencies, module, &mut work) {
                return Self::retain_all(program);
            }
        }

        let live_exports = loop {
            while let Some(fact) = work.pop() {
                match fact {
                    ReachabilityFact::EvaluateModule(module) => {
                        if !evaluated_modules.insert(module) {
                            continue;
                        }

                        let program_module = program
                            .module(module)
                            .expect("evaluated module must remain live");

                        if !dependencies.has_complete_linkage(module, program_module.ir()) {
                            return Self::retain_all(program);
                        }

                        for dependency in dependencies.outgoing(module) {
                            if let ModuleTarget::Internal(target) = dependency.target() {
                                work.push(ReachabilityFact::EvaluateModule(*target));

                                if matches!(
                                    dependency.request().kind(),
                                    ModuleRequestKind::DynamicImport
                                        | ModuleRequestKind::CommonJsRequire
                                ) {
                                    work.push(ReachabilityFact::UseNamespace(*target));
                                }
                            }
                        }

                        for (_, function) in program_module.ir().functions() {
                            for (_, operation) in function.operations() {
                                match operation.kind() {
                                    OperationKind::LoadBinding(load) => {
                                        work.push(ReachabilityFact::UseBinding(
                                            ProgramBindingId::new(module, load.binding()),
                                        ));
                                    }
                                    OperationKind::LoadGlobal(global)
                                        if global.name() == "eval" =>
                                    {
                                        for (binding, _) in program_module.ir().bindings() {
                                            work.push(ReachabilityFact::UseBinding(
                                                ProgramBindingId::new(module, binding),
                                            ));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    ReachabilityFact::UseBinding(binding) => {
                        if !live_bindings.insert(binding) {
                            continue;
                        }

                        match linkage.imported_binding(binding) {
                            None => {}
                            Some(ImportedBindingTarget::Binding(target)) => {
                                work.push(ReachabilityFact::UseBinding(*target));
                            }
                            Some(ImportedBindingTarget::Namespace(ModuleTarget::Internal(
                                module,
                            ))) => {
                                work.push(ReachabilityFact::UseNamespace(*module));
                            }
                            Some(ImportedBindingTarget::Namespace(
                                ModuleTarget::Opaque(_) | ModuleTarget::External(_),
                            ))
                            | Some(ImportedBindingTarget::OpaqueExport { .. })
                            | Some(ImportedBindingTarget::ExternalExport { .. }) => {}
                            Some(ImportedBindingTarget::Unresolved) => {
                                return Self::retain_all(program);
                            }
                        }
                    }
                    ReachabilityFact::UseNamespace(module) => {
                        if !live_namespaces.insert(module) {
                            continue;
                        }

                        work.push(ReachabilityFact::EvaluateModule(module));

                        let module_ir = program
                            .module(module)
                            .expect("used namespace module must remain live")
                            .ir();

                        for (binding, _) in module_ir.bindings() {
                            work.push(ReachabilityFact::UseBinding(ProgramBindingId::new(
                                module, binding,
                            )));
                        }
                    }
                }
            }

            let (live_exports, required_bindings) =
                compute_live_exports(program, &dependencies, &live_bindings, &live_namespaces);
            let mut added_binding = false;

            for binding in required_bindings {
                if !live_bindings.contains(&binding) {
                    work.push(ReachabilityFact::UseBinding(binding));
                    added_binding = true;
                }
            }

            if !added_binding {
                break live_exports;
            }
        };

        Self {
            evaluated_modules,
            live_bindings,
            live_exports,
        }
    }

    /// Returns whether executing the program can require evaluating `module`.
    pub fn is_module_evaluated(&self, module: ModuleId) -> bool {
        self.evaluated_modules.contains(&module)
    }

    /// Returns whether `binding` can be observed by the linked program.
    pub fn is_binding_live(&self, binding: ProgramBindingId) -> bool {
        self.live_bindings.contains(&binding)
    }

    /// Returns the number of evaluated modules.
    pub fn evaluated_module_count(&self) -> usize {
        self.evaluated_modules.len()
    }

    /// Returns the number of live program bindings.
    pub fn live_binding_count(&self) -> usize {
        self.live_bindings.len()
    }

    /// Returns whether one source-order export is observable.
    pub(crate) fn is_export_live(&self, module: ModuleId, export: usize) -> bool {
        self.live_exports
            .contains(&ProgramExport { module, export })
    }

    fn retain_all(program: &JsProgramIr) -> Self {
        Self {
            evaluated_modules: program.modules().map(|(module, _)| module).collect(),
            live_bindings: program
                .modules()
                .flat_map(|(module, data)| {
                    data.ir()
                        .bindings()
                        .map(move |(binding, _)| ProgramBindingId::new(module, binding))
                })
                .collect(),
            live_exports: program
                .modules()
                .flat_map(|(module, data)| {
                    (0..data.ir().exports().len())
                        .map(move |export| ProgramExport { module, export })
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ProgramExport {
    module: ModuleId,
    export: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ReachabilityFact {
    EvaluateModule(ModuleId),
    UseBinding(ProgramBindingId),
    UseNamespace(ModuleId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ExportDemand {
    Name { module: ModuleId, name: Box<str> },
    Namespace(ModuleId),
}

fn compute_live_exports(
    program: &JsProgramIr,
    dependencies: &DependencyIndex<'_>,
    live_bindings: &FxHashSet<ProgramBindingId>,
    live_namespaces: &FxHashSet<ModuleId>,
) -> (FxHashSet<ProgramExport>, FxHashSet<ProgramBindingId>) {
    let mut live_exports = FxHashSet::default();
    let mut required_bindings = FxHashSet::default();
    let mut demands = FxHashSet::default();
    let mut work = VecDeque::new();

    for module in program
        .entry_modules()
        .chain(live_namespaces.iter().copied())
    {
        push_export_demand(ExportDemand::Namespace(module), &mut demands, &mut work);
    }

    for (module, data) in program.modules() {
        for import in data.ir().imports() {
            let Some(binding) = import.binding() else {
                continue;
            };
            if !live_bindings.contains(&ProgramBindingId::new(module, binding)) {
                continue;
            }
            let Some(ModuleTarget::Internal(target)) = dependencies.unique_target(
                module,
                ModuleRequestKind::StaticImport,
                import.source(),
                import.attributes(),
            ) else {
                continue;
            };

            let demand = match import {
                ModuleImport::Bare { .. } => continue,
                ModuleImport::Default { .. } => ExportDemand::Name {
                    module: *target,
                    name: "default".into(),
                },
                ModuleImport::Namespace { .. } => ExportDemand::Namespace(*target),
                ModuleImport::Named { imported, .. } => ExportDemand::Name {
                    module: *target,
                    name: imported.as_str().into(),
                },
            };
            push_export_demand(demand, &mut demands, &mut work);
        }
    }

    while let Some(demand) = work.pop_front() {
        match demand {
            ExportDemand::Name { module, name } => {
                let module_ir = program
                    .module(module)
                    .expect("demanded export module must remain live")
                    .ir();

                if let Some((export, data)) =
                    module_ir.exports().iter().enumerate().find(|(_, export)| {
                        export
                            .exported_name()
                            .is_some_and(|exported| exported.as_str() == name.as_ref())
                    })
                {
                    live_exports.insert(ProgramExport { module, export });
                    if let ModuleExport::Local { binding, .. } = data {
                        required_bindings.insert(ProgramBindingId::new(module, *binding));
                    }
                    push_forwarded_export_demand(
                        module,
                        data,
                        dependencies,
                        &mut demands,
                        &mut work,
                    );
                    continue;
                }

                if name.as_ref() == "default" {
                    continue;
                }

                for (export, data) in module_ir.exports().iter().enumerate() {
                    let ModuleExport::Star {
                        source, attributes, ..
                    } = data
                    else {
                        continue;
                    };
                    live_exports.insert(ProgramExport { module, export });
                    push_internal_export_demand(
                        module,
                        source,
                        attributes,
                        ExportTarget::Name(name.clone()),
                        dependencies,
                        &mut demands,
                        &mut work,
                    );
                }
            }
            ExportDemand::Namespace(module) => {
                let module_ir = program
                    .module(module)
                    .expect("demanded namespace module must remain live")
                    .ir();

                for (export, data) in module_ir.exports().iter().enumerate() {
                    live_exports.insert(ProgramExport { module, export });
                    if let ModuleExport::Local { binding, .. } = data {
                        required_bindings.insert(ProgramBindingId::new(module, *binding));
                    }
                    push_forwarded_export_demand(
                        module,
                        data,
                        dependencies,
                        &mut demands,
                        &mut work,
                    );
                }
            }
        }
    }

    (live_exports, required_bindings)
}

fn push_forwarded_export_demand(
    module: ModuleId,
    export: &ModuleExport,
    dependencies: &DependencyIndex<'_>,
    demands: &mut FxHashSet<ExportDemand>,
    work: &mut VecDeque<ExportDemand>,
) {
    match export {
        ModuleExport::Empty { .. } | ModuleExport::Local { .. } => {}
        ModuleExport::Indirect {
            source,
            attributes,
            imported,
            ..
        } => push_internal_export_demand(
            module,
            source,
            attributes,
            ExportTarget::Name(imported.as_str().into()),
            dependencies,
            demands,
            work,
        ),
        ModuleExport::Namespace {
            source, attributes, ..
        }
        | ModuleExport::Star {
            source, attributes, ..
        } => push_internal_export_demand(
            module,
            source,
            attributes,
            ExportTarget::Namespace,
            dependencies,
            demands,
            work,
        ),
    }
}

enum ExportTarget {
    Name(Box<str>),
    Namespace,
}

fn push_internal_export_demand(
    module: ModuleId,
    source: &str,
    attributes: &[ModuleAttribute],
    target: ExportTarget,
    dependencies: &DependencyIndex<'_>,
    demands: &mut FxHashSet<ExportDemand>,
    work: &mut VecDeque<ExportDemand>,
) {
    let Some(ModuleTarget::Internal(module)) =
        dependencies.unique_target(module, ModuleRequestKind::ReExport, source, attributes)
    else {
        return;
    };

    let demand = match target {
        ExportTarget::Name(name) => ExportDemand::Name {
            module: *module,
            name,
        },
        ExportTarget::Namespace => ExportDemand::Namespace(*module),
    };
    push_export_demand(demand, demands, work);
}

fn push_export_demand(
    demand: ExportDemand,
    demands: &mut FxHashSet<ExportDemand>,
    work: &mut VecDeque<ExportDemand>,
) {
    if demands.insert(demand.clone()) {
        work.push_back(demand);
    }
}

fn retain_entry_exports(
    program: &JsProgramIr,
    dependencies: &DependencyIndex<'_>,
    module: ModuleId,
    work: &mut WorkQueue<ReachabilityFact>,
) -> bool {
    let module_ir = program
        .module(module)
        .expect("entry module must remain live")
        .ir();

    for export in module_ir.exports() {
        if matches!(export, ModuleExport::Empty { .. }) {
            continue;
        }

        if let Some(binding) = export.binding() {
            work.push(ReachabilityFact::UseBinding(ProgramBindingId::new(
                module, binding,
            )));
            continue;
        }

        let Some(source) = export.source() else {
            continue;
        };
        let Some(target) = dependencies.unique_target(
            module,
            ModuleRequestKind::ReExport,
            source,
            export.attributes(),
        ) else {
            return false;
        };

        if let ModuleTarget::Internal(target) = target {
            // Entry-module signatures are externally observable. Until export
            // paths are represented explicitly, retaining the target namespace
            // is the conservative treatment for every form of re-export.
            work.push(ReachabilityFact::UseNamespace(*target));
        }
    }

    true
}

struct DependencyIndex<'program> {
    by_importer: FxHashMap<ModuleId, Vec<&'program ModuleDependency>>,
}

impl<'program> DependencyIndex<'program> {
    fn new(program: &'program JsProgramIr) -> Self {
        let mut by_importer = FxHashMap::default();

        for dependency in program.dependencies() {
            by_importer
                .entry(dependency.importer())
                .or_insert_with(Vec::new)
                .push(dependency);
        }

        Self { by_importer }
    }

    fn outgoing(&self, importer: ModuleId) -> &[&'program ModuleDependency] {
        self.by_importer
            .get(&importer)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    fn has_complete_linkage(&self, importer: ModuleId, module: &evrel_js_ir::JsModuleIr) -> bool {
        let static_requests_are_resolved = module.imports().iter().all(|import| {
            self.has_unique_target(
                importer,
                ModuleRequestKind::StaticImport,
                import.source(),
                import.attributes(),
            )
        }) && module.exports().iter().all(|export| {
            export.source().is_none_or(|source| {
                self.has_unique_target(
                    importer,
                    ModuleRequestKind::ReExport,
                    source,
                    export.attributes(),
                )
            })
        });

        static_requests_are_resolved
            && module
                .functions()
                .all(|(_, function)| self.has_complete_dynamic_linkage(importer, function))
    }

    fn has_complete_dynamic_linkage(
        &self,
        importer: ModuleId,
        function: &evrel_js_ir::JsFunctionIr,
    ) -> bool {
        function.operations().all(|(_, operation)| {
            let OperationKind::DynamicImport(dynamic_import) = operation.kind() else {
                return true;
            };

            if dynamic_import.has_options() {
                return false;
            }

            let Some(specifier) = operation
                .operands()
                .first()
                .and_then(|value| function.value(*value))
                .and_then(|value| match value.definition() {
                    ValueDefinition::OperationResult { operation, .. } => {
                        function.operation(*operation)
                    }
                    ValueDefinition::FunctionParameter { .. }
                    | ValueDefinition::BlockParameter { .. } => None,
                })
                .and_then(|operation| match operation.kind() {
                    OperationKind::Constant(constant) => match constant.value() {
                        ConstantValue::String(specifier) => Some(specifier.as_str()),
                        _ => None,
                    },
                    _ => None,
                })
            else {
                return false;
            };

            self.has_unique_target(importer, ModuleRequestKind::DynamicImport, specifier, &[])
        })
    }

    fn has_unique_target(
        &self,
        importer: ModuleId,
        kind: ModuleRequestKind,
        specifier: &str,
        attributes: &[ModuleAttribute],
    ) -> bool {
        self.unique_target(importer, kind, specifier, attributes)
            .is_some()
    }

    fn unique_target(
        &self,
        importer: ModuleId,
        kind: ModuleRequestKind,
        specifier: &str,
        attributes: &[ModuleAttribute],
    ) -> Option<&'program ModuleTarget> {
        let mut targets = self.outgoing(importer).iter().filter_map(|dependency| {
            let request = dependency.request();

            (request.kind() == kind
                && request.specifier() == specifier
                && request.attributes() == attributes)
                .then_some(dependency.target())
        });

        let target = targets.next()?;

        targets
            .all(|candidate| candidate == target)
            .then_some(target)
    }
}

#[cfg(test)]
mod tests {
    use evrel_js_ir::{
        BindingKind, ConstantOp, ConstantValue, DynamicImportOp, DynamicImportPhase, JsModuleIr,
        JsProgramIr, JsString, LoadBindingOp, LocationId, ModuleBuilder, ModuleDependency,
        ModuleExport, ModuleExportName, ModuleImport, ModuleKey, ModuleRequest, ModuleRequestKind,
        ModuleTarget, OperationKind, ProgramBindingId, UnwindTarget,
    };

    use super::{ProgramLinkage, ProgramReachability};

    fn compute(program: &JsProgramIr) -> ProgramReachability {
        let linkage = ProgramLinkage::analyze(program);

        ProgramReachability::compute(program, &linkage)
    }

    #[test]
    fn follows_internal_dependencies_from_entry_modules() {
        let mut program = JsProgramIr::new();
        let entry = program.add_module(ModuleKey::new("entry"), JsModuleIr::new());
        let dependency = program.add_module(ModuleKey::new("dependency"), JsModuleIr::new());
        let disconnected = program.add_module(ModuleKey::new("disconnected"), JsModuleIr::new());

        program.add_entry_module(entry);
        program.add_dependency(ModuleDependency::new(
            entry,
            ModuleRequest::new(ModuleRequestKind::DynamicImport, "./dependency.js", []),
            ModuleTarget::Internal(dependency),
        ));

        let reachability = compute(&program);

        assert!(reachability.is_module_evaluated(entry));
        assert!(reachability.is_module_evaluated(dependency));
        assert!(!reachability.is_module_evaluated(disconnected));
    }

    #[test]
    fn retains_every_module_when_static_linkage_is_incomplete() {
        let mut entry_ir = JsModuleIr::new();
        ModuleBuilder::new(&mut entry_ir).add_import(ModuleImport::bare(
            LocationId::UNKNOWN,
            "./missing.js",
            [],
        ));

        let mut program = JsProgramIr::new();
        let entry = program.add_module(ModuleKey::new("entry"), entry_ir);
        let possible_target =
            program.add_module(ModuleKey::new("possible-target"), JsModuleIr::new());
        program.add_entry_module(entry);

        let reachability = compute(&program);

        assert!(reachability.is_module_evaluated(entry));
        assert!(reachability.is_module_evaluated(possible_target));
    }

    #[test]
    fn retains_every_module_when_a_dynamic_import_is_unresolved() {
        let mut entry_ir = JsModuleIr::new();
        let entry_function = entry_ir.entry_function();
        let mut module_builder = ModuleBuilder::new(&mut entry_ir);
        let mut builder = module_builder.function_builder(entry_function);
        let specifier = builder.append_operation(
            LocationId::UNKNOWN,
            OperationKind::Constant(ConstantOp::new(ConstantValue::String(JsString::new(
                "./missing.js",
                false,
            )))),
            [],
            UnwindTarget::Propagate,
        );
        let specifier = builder.operation_results(specifier)[0];
        builder.append_operation(
            LocationId::UNKNOWN,
            OperationKind::DynamicImport(DynamicImportOp::new(
                DynamicImportPhase::Evaluation,
                false,
            )),
            [specifier],
            UnwindTarget::Propagate,
        );

        let mut program = JsProgramIr::new();
        let entry = program.add_module(ModuleKey::new("entry"), entry_ir);
        let possible_target =
            program.add_module(ModuleKey::new("possible-target"), JsModuleIr::new());
        program.add_entry_module(entry);

        let reachability = compute(&program);

        assert!(reachability.is_module_evaluated(entry));
        assert!(reachability.is_module_evaluated(possible_target));
    }

    #[test]
    fn follows_a_used_import_to_its_exported_binding() {
        let mut dependency_ir = JsModuleIr::new();
        let dependency_entry = dependency_ir.entry_function();
        let dependency_binding = {
            let mut builder = ModuleBuilder::new(&mut dependency_ir);
            let binding = builder.create_binding(dependency_entry, "value", BindingKind::Const);
            builder.add_export(ModuleExport::local(
                LocationId::UNKNOWN,
                ModuleExportName::Identifier("value".into()),
                binding,
            ));
            binding
        };

        let mut entry_ir = JsModuleIr::new();
        let entry_function = entry_ir.entry_function();
        let imported_binding = {
            let mut builder = ModuleBuilder::new(&mut entry_ir);
            let binding = builder.create_binding(entry_function, "value", BindingKind::Import);
            builder.add_import(ModuleImport::named(
                LocationId::UNKNOWN,
                "./dependency.js",
                [],
                ModuleExportName::Identifier("value".into()),
                binding,
            ));
            builder.function_builder(entry_function).append_operation(
                LocationId::UNKNOWN,
                OperationKind::LoadBinding(LoadBindingOp::new(binding)),
                [],
                UnwindTarget::Propagate,
            );
            binding
        };

        let mut program = JsProgramIr::new();
        let entry = program.add_module(ModuleKey::new("entry"), entry_ir);
        let dependency = program.add_module(ModuleKey::new("dependency"), dependency_ir);
        program.add_entry_module(entry);
        program.add_dependency(ModuleDependency::new(
            entry,
            ModuleRequest::new(ModuleRequestKind::StaticImport, "./dependency.js", []),
            ModuleTarget::Internal(dependency),
        ));

        let reachability = compute(&program);

        assert_eq!(reachability.evaluated_module_count(), 2);
        assert_eq!(reachability.live_binding_count(), 2);
        assert!(reachability.is_binding_live(ProgramBindingId::new(entry, imported_binding)));
        assert!(
            reachability.is_binding_live(ProgramBindingId::new(dependency, dependency_binding))
        );
    }

    #[test]
    fn evaluates_an_imported_module_without_marking_unused_bindings_live() {
        let mut dependency_ir = JsModuleIr::new();
        let dependency_entry = dependency_ir.entry_function();
        let dependency_binding = ModuleBuilder::new(&mut dependency_ir).create_binding(
            dependency_entry,
            "unused",
            BindingKind::Const,
        );

        let mut entry_ir = JsModuleIr::new();
        let entry_function = entry_ir.entry_function();
        let imported_binding = {
            let mut builder = ModuleBuilder::new(&mut entry_ir);
            let binding = builder.create_binding(entry_function, "unused", BindingKind::Import);
            builder.add_import(ModuleImport::named(
                LocationId::UNKNOWN,
                "./dependency.js",
                [],
                ModuleExportName::Identifier("unused".into()),
                binding,
            ));
            binding
        };

        let mut program = JsProgramIr::new();
        let entry = program.add_module(ModuleKey::new("entry"), entry_ir);
        let dependency = program.add_module(ModuleKey::new("dependency"), dependency_ir);
        program.add_entry_module(entry);
        program.add_dependency(ModuleDependency::new(
            entry,
            ModuleRequest::new(ModuleRequestKind::StaticImport, "./dependency.js", []),
            ModuleTarget::Internal(dependency),
        ));

        let reachability = compute(&program);

        assert_eq!(reachability.evaluated_module_count(), 2);
        assert_eq!(reachability.live_binding_count(), 0);
        assert!(reachability.is_module_evaluated(dependency));
        assert!(!reachability.is_binding_live(ProgramBindingId::new(entry, imported_binding)));
        assert!(
            !reachability.is_binding_live(ProgramBindingId::new(dependency, dependency_binding))
        );
    }
}
