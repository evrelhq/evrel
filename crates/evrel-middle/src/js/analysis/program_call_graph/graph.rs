use std::collections::{BTreeMap, BTreeSet};

use evrel_js_ir::{
    CallTarget, JsProgramIr, OperationKind, ProgramBindingId, ProgramFunctionId,
    ProgramOperationId, ValueId,
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::super::program_linkage::ProgramLinkage;
use super::targets::ProgramFunctionTargets;

/// Dense identifier for one invocation in a [`ProgramCallGraph`] snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CallSiteId(u32);

impl CallSiteId {
    /// Returns the call site's stable position in its analysis snapshot.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    fn from_index(index: usize) -> Self {
        Self(u32::try_from(index).expect("program call-site count must fit in u32"))
    }
}

/// JavaScript invocation form represented by a call-graph site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallSiteKind {
    /// An ordinary JavaScript call, including property calls.
    Call,

    /// A JavaScript constructor invocation.
    Construct,

    /// An implicit superclass-constructor invocation.
    SuperCall,

    /// Invocation of a tagged-template function.
    TaggedTemplate,
}

/// Whether a target set accounts for every runtime callee.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallTargetCompleteness {
    /// Every runtime target is represented.
    Complete,

    /// Additional runtime targets may exist.
    Incomplete,
}

/// Conservatively resolved program functions for one invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallTargetSet {
    functions: Box<[ProgramFunctionId]>,
    completeness: CallTargetCompleteness,
}

impl CallTargetSet {
    pub(super) fn new(
        functions: impl Into<Box<[ProgramFunctionId]>>,
        completeness: CallTargetCompleteness,
    ) -> Self {
        Self {
            functions: functions.into(),
            completeness,
        }
    }

    pub(super) fn unknown() -> Self {
        Self::new([], CallTargetCompleteness::Incomplete)
    }

    /// Returns statically known program functions in stable ID order.
    pub fn functions(&self) -> &[ProgramFunctionId] {
        &self.functions
    }

    /// Returns whether the known target list is exhaustive.
    pub const fn completeness(&self) -> CallTargetCompleteness {
        self.completeness
    }

    /// Returns the sole target when it is exact and exhaustive.
    pub fn exact_function(&self) -> Option<ProgramFunctionId> {
        (self.completeness == CallTargetCompleteness::Complete && self.functions.len() == 1)
            .then(|| self.functions[0])
    }
}

/// One explicit JavaScript invocation and its conservative target set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSite {
    id: CallSiteId,
    operation: ProgramOperationId,
    kind: CallSiteKind,
    targets: CallTargetSet,
}

impl CallSite {
    /// Returns this site's dense analysis-local identifier.
    pub const fn id(&self) -> CallSiteId {
        self.id
    }

    /// Returns the function containing this invocation.
    pub const fn caller(&self) -> ProgramFunctionId {
        self.operation.function()
    }

    /// Returns the invocation operation.
    pub const fn operation(&self) -> ProgramOperationId {
        self.operation
    }

    /// Returns the JavaScript invocation form.
    pub const fn kind(&self) -> CallSiteKind {
        self.kind
    }

    /// Returns the conservative program target set.
    pub const fn targets(&self) -> &CallTargetSet {
        &self.targets
    }
}

/// Where a program function identity is referenced without a resolved call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunctionReferenceSite {
    /// The host can execute a configured program entry module.
    ProgramEntry,

    /// The operation allocates the referenced function object.
    Allocation { operation: ProgramOperationId },

    /// The operation embeds a method, accessor, or class-owned body.
    Embedded { operation: ProgramOperationId },

    /// An ordinary operand use that is not a resolved invocation target.
    ValueUse {
        operation: ProgramOperationId,
        operand_index: u32,
    },

    /// A module export exposes the function identity beyond local calls.
    Export { binding: ProgramBindingId },

    /// Direct eval can observe and invoke a visible binding.
    DirectEval {
        operation: ProgramOperationId,
        binding: ProgramBindingId,
    },
}

/// One statically known reference to a program-owned function body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionReference {
    target: ProgramFunctionId,
    site: FunctionReferenceSite,
}

impl FunctionReference {
    pub(super) const fn new(target: ProgramFunctionId, site: FunctionReferenceSite) -> Self {
        Self { target, site }
    }

    /// Returns the referenced function.
    pub const fn target(&self) -> ProgramFunctionId {
        self.target
    }

    /// Returns where the function identity is referenced.
    pub const fn site(&self) -> FunctionReferenceSite {
        self.site
    }
}

/// Whole-program call and function-reference topology for one immutable IR snapshot.
///
/// Every incoming path not represented by a [`CallSite`] remains visible as a
/// non-allocation [`FunctionReference`]. Consumers can therefore inspect why a
/// function lacks complete incoming-call information instead of treating all
/// indirect behavior as one opaque flag.
///
/// Recompute after changing functions, binding linkage, invocation operands,
/// control-flow forwarding, or any use of a function object.
#[derive(Debug, Clone)]
pub struct ProgramCallGraph {
    sites: Box<[CallSite]>,
    sites_by_caller: FxHashMap<ProgramFunctionId, Box<[CallSiteId]>>,
    callers_by_callee: FxHashMap<ProgramFunctionId, Box<[CallSiteId]>>,
    references: Box<[FunctionReference]>,
    references_by_target: FxHashMap<ProgramFunctionId, Box<[usize]>>,
    complete_incoming: FxHashSet<ProgramFunctionId>,
}

impl ProgramCallGraph {
    /// Analyzes calls and function references across one linked program.
    pub fn analyze(js_program: &JsProgramIr, linkage: &ProgramLinkage) -> Self {
        let targets = ProgramFunctionTargets::analyze(js_program, linkage);
        let mut sites = Vec::new();
        let mut sites_by_caller = BTreeMap::<ProgramFunctionId, Vec<CallSiteId>>::new();
        let mut callers_by_callee = BTreeMap::<ProgramFunctionId, Vec<CallSiteId>>::new();
        let mut invocation_uses = FxHashSet::default();

        for (module, program_module) in js_program.modules() {
            for (function, function_ir) in program_module.ir().functions() {
                let caller = ProgramFunctionId::new(module, function);

                for (operation, data) in function_ir.operations() {
                    let Some((kind, target_operand)) = invocation(data.kind(), data.operands())
                    else {
                        continue;
                    };
                    let operation = ProgramOperationId::new(caller, operation);
                    let target_set = target_operand
                        .map(|(_, value)| targets.target_set(caller, value))
                        .unwrap_or_else(CallTargetSet::unknown);
                    let id = CallSiteId::from_index(sites.len());

                    if let Some((operand_index, _)) = target_operand {
                        invocation_uses.insert((operation, operand_index));
                    }
                    for target in target_set.functions() {
                        callers_by_callee.entry(*target).or_default().push(id);
                    }
                    sites_by_caller.entry(caller).or_default().push(id);
                    sites.push(CallSite {
                        id,
                        operation,
                        kind,
                        targets: target_set,
                    });
                }
            }
        }

        let mut references = collect_references(js_program, &targets, &invocation_uses);
        references.extend(js_program.entry_modules().filter_map(|module| {
            let entry = js_program.module(module)?.ir().entry_function();
            Some(FunctionReference::new(
                ProgramFunctionId::new(module, entry),
                FunctionReferenceSite::ProgramEntry,
            ))
        }));
        references.extend(targets.direct_eval_references());
        references.sort_unstable_by_key(reference_sort_key);
        references.dedup();

        let mut references_by_target = BTreeMap::<ProgramFunctionId, Vec<usize>>::new();
        for (index, reference) in references.iter().enumerate() {
            references_by_target
                .entry(reference.target())
                .or_default()
                .push(index);
        }

        let mut complete_incoming = js_program
            .modules()
            .flat_map(|(module, program_module)| {
                program_module
                    .ir()
                    .functions()
                    .map(move |(function, _)| ProgramFunctionId::new(module, function))
            })
            .collect::<BTreeSet<_>>();

        for reference in &references {
            if !matches!(reference.site(), FunctionReferenceSite::Allocation { .. }) {
                complete_incoming.remove(&reference.target());
            }
        }

        Self {
            sites: sites.into_boxed_slice(),
            sites_by_caller: freeze_index(sites_by_caller),
            callers_by_callee: freeze_index(callers_by_callee),
            references: references.into_boxed_slice(),
            references_by_target: freeze_index(references_by_target),
            complete_incoming: complete_incoming.into_iter().collect(),
        }
    }

    /// Iterates every explicit invocation in deterministic program order.
    pub fn sites(&self) -> impl ExactSizeIterator<Item = &CallSite> {
        self.sites.iter()
    }

    /// Returns one invocation site from this analysis snapshot.
    pub fn site(&self, id: CallSiteId) -> Option<&CallSite> {
        self.sites.get(id.index())
    }

    /// Iterates invocation sites contained in `caller`.
    pub fn sites_in(&self, caller: ProgramFunctionId) -> impl Iterator<Item = &CallSite> {
        self.sites_by_caller
            .get(&caller)
            .into_iter()
            .flatten()
            .map(|id| &self.sites[id.index()])
    }

    /// Iterates known invocation sites that may call `callee`.
    pub fn callers_of(&self, callee: ProgramFunctionId) -> impl Iterator<Item = &CallSite> {
        self.callers_by_callee
            .get(&callee)
            .into_iter()
            .flatten()
            .map(|id| &self.sites[id.index()])
    }

    /// Iterates every statically known non-call function reference.
    pub fn references(&self) -> impl ExactSizeIterator<Item = &FunctionReference> {
        self.references.iter()
    }

    /// Iterates non-call references to `target`.
    pub fn references_to(
        &self,
        target: ProgramFunctionId,
    ) -> impl Iterator<Item = &FunctionReference> {
        self.references_by_target
            .get(&target)
            .into_iter()
            .flatten()
            .map(|index| &self.references[*index])
    }

    /// Returns whether every possible invocation of `function` is represented.
    ///
    /// A program entry or any other non-allocation reference may invoke the
    /// function through a path that is not represented by a call site.
    pub fn has_complete_incoming_calls(&self, function: ProgramFunctionId) -> bool {
        self.complete_incoming.contains(&function)
    }
}

fn collect_references(
    js_program: &JsProgramIr,
    targets: &ProgramFunctionTargets,
    invocation_uses: &FxHashSet<(ProgramOperationId, u32)>,
) -> Vec<FunctionReference> {
    let mut references = Vec::new();

    for (module, program_module) in js_program.modules() {
        let module_ir = program_module.ir();

        for (function, function_ir) in module_ir.functions() {
            let owner = ProgramFunctionId::new(module, function);

            for (operation_id, operation) in function_ir.operations() {
                let operation_id = ProgramOperationId::new(owner, operation_id);
                match operation.kind() {
                    OperationKind::CreateFunction(create) => {
                        references.push(FunctionReference {
                            target: ProgramFunctionId::new(module, create.function()),
                            site: FunctionReferenceSite::Allocation {
                                operation: operation_id,
                            },
                        });
                    }
                    kind => kind.visit_referenced_functions(|target| {
                        references.push(FunctionReference {
                            target: ProgramFunctionId::new(module, target),
                            site: FunctionReferenceSite::Embedded {
                                operation: operation_id,
                            },
                        });
                    }),
                }
            }

            for (value, value_data) in function_ir.values() {
                let target_set = targets.target_set(owner, value);
                if target_set.functions().is_empty() {
                    continue;
                }

                for use_site in value_data.uses() {
                    let operation = ProgramOperationId::new(owner, use_site.operation());
                    let key = (operation, use_site.operand_index());
                    if invocation_uses.contains(&key) || targets.is_forwarding_use(key) {
                        continue;
                    }

                    references.extend(target_set.functions().iter().map(|target| {
                        FunctionReference {
                            target: *target,
                            site: FunctionReferenceSite::ValueUse {
                                operation,
                                operand_index: use_site.operand_index(),
                            },
                        }
                    }));
                }
            }
        }

        for export in module_ir.exports() {
            let Some(binding) = export.binding() else {
                continue;
            };
            let binding = ProgramBindingId::new(module, binding);
            references.extend(targets.binding_targets(binding).iter().map(|target| {
                FunctionReference {
                    target: *target,
                    site: FunctionReferenceSite::Export { binding },
                }
            }));
        }
    }

    references
}

fn invocation(
    kind: &OperationKind,
    operands: &[ValueId],
) -> Option<(CallSiteKind, Option<(u32, ValueId)>)> {
    let value_target = |kind, index: Option<usize>| {
        Some((
            kind,
            index.map(|index| {
                (
                    u32::try_from(index).expect("operand index must fit in u32"),
                    operands[index],
                )
            }),
        ))
    };

    match kind {
        OperationKind::Call(call) => value_target(CallSiteKind::Call, call.callee_operand_index()),
        OperationKind::Construct(construct) => value_target(
            CallSiteKind::Construct,
            Some(construct.constructor_operand_index()),
        ),
        OperationKind::SuperCall(_) => Some((CallSiteKind::SuperCall, None)),
        OperationKind::TaggedTemplate(template) => value_target(
            CallSiteKind::TaggedTemplate,
            match template.target() {
                CallTarget::Value { .. } => Some(0),
                CallTarget::Property(_) | CallTarget::SuperProperty(_) => None,
            },
        ),
        _ => None,
    }
}

fn freeze_index<Key, Value>(index: BTreeMap<Key, Vec<Value>>) -> FxHashMap<Key, Box<[Value]>>
where
    Key: std::hash::Hash + Eq,
{
    index
        .into_iter()
        .map(|(key, values)| (key, values.into_boxed_slice()))
        .collect()
}

fn reference_sort_key(
    reference: &FunctionReference,
) -> (
    ProgramFunctionId,
    u8,
    Option<ProgramOperationId>,
    u32,
    Option<ProgramBindingId>,
) {
    let (kind, operation, operand, binding) = match reference.site() {
        FunctionReferenceSite::ProgramEntry => (0, None, 0, None),
        FunctionReferenceSite::Allocation { operation } => (1, Some(operation), 0, None),
        FunctionReferenceSite::Embedded { operation } => (2, Some(operation), 0, None),
        FunctionReferenceSite::ValueUse {
            operation,
            operand_index,
        } => (3, Some(operation), operand_index, None),
        FunctionReferenceSite::Export { binding } => (4, None, 0, Some(binding)),
        FunctionReferenceSite::DirectEval { operation, binding } => {
            (5, Some(operation), 0, Some(binding))
        }
    };

    (reference.target(), kind, operation, operand, binding)
}
