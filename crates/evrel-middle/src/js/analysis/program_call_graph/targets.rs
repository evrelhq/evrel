use std::collections::BTreeSet;

use evrel_js_ir::{
    BindingKind, BlockParameterSource, JsFunctionIr, JsProgramIr, OperationKind, ProgramBindingId,
    ProgramFunctionId, ProgramOperationId, ValueDefinition, ValueId,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::js::work_queue::WorkQueue;

use super::super::direct_eval::is_direct_eval_call;
use super::super::program_linkage::{ImportedBindingTarget, ProgramLinkage};
use super::{CallTargetCompleteness, CallTargetSet, FunctionReference, FunctionReferenceSite};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ProgramValue {
    function: ProgramFunctionId,
    value: ValueId,
}

impl ProgramValue {
    const fn new(function: ProgramFunctionId, value: ValueId) -> Self {
        Self { function, value }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TargetNode {
    Value(ProgramValue),
    Binding(ProgramBindingId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TargetState {
    Bottom,
    Known {
        functions: BTreeSet<ProgramFunctionId>,
        completeness: CallTargetCompleteness,
    },
}

impl TargetState {
    fn bottom() -> Self {
        Self::Bottom
    }

    fn unknown() -> Self {
        Self::Known {
            functions: BTreeSet::new(),
            completeness: CallTargetCompleteness::Incomplete,
        }
    }

    fn exact(function: ProgramFunctionId) -> Self {
        Self::Known {
            functions: [function].into(),
            completeness: CallTargetCompleteness::Complete,
        }
    }

    fn join(&mut self, incoming: &Self) -> bool {
        let Self::Known {
            functions: incoming_functions,
            completeness: incoming_completeness,
        } = incoming
        else {
            return false;
        };

        match self {
            Self::Bottom => {
                *self = incoming.clone();
                true
            }
            Self::Known {
                functions,
                completeness,
            } => {
                let previous_len = functions.len();
                let previous_completeness = *completeness;
                functions.extend(incoming_functions.iter().copied());
                if *incoming_completeness == CallTargetCompleteness::Incomplete {
                    *completeness = CallTargetCompleteness::Incomplete;
                }

                previous_len != functions.len() || previous_completeness != *completeness
            }
        }
    }

    fn as_target_set(&self) -> CallTargetSet {
        match self {
            Self::Bottom => CallTargetSet::unknown(),
            Self::Known {
                functions,
                completeness,
            } => CallTargetSet::new(
                functions.iter().copied().collect::<Box<[_]>>(),
                *completeness,
            ),
        }
    }
}

#[derive(Debug, Clone)]
struct DirectEvalExposure {
    operation: ProgramOperationId,
    bindings: Box<[ProgramBindingId]>,
}

/// Fixed-point function-target facts shared by graph construction.
pub(super) struct ProgramFunctionTargets {
    states: FxHashMap<TargetNode, TargetState>,
    forwarding_uses: FxHashSet<(ProgramOperationId, u32)>,
    direct_eval: Box<[DirectEvalExposure]>,
}

impl ProgramFunctionTargets {
    pub(super) fn analyze(js_program: &JsProgramIr, linkage: &ProgramLinkage) -> Self {
        let mut solver = TargetSolver::new();

        for (module, program_module) in js_program.modules() {
            let module_ir = program_module.ir();

            for (binding, data) in module_ir.bindings() {
                let binding = ProgramBindingId::new(module, binding);
                match data.kind() {
                    BindingKind::Import => match linkage.imported_binding(binding) {
                        Some(ImportedBindingTarget::Binding(target)) => {
                            solver.add_dependency(
                                TargetNode::Binding(*target),
                                TargetNode::Binding(binding),
                            );
                        }
                        Some(
                            ImportedBindingTarget::Namespace(_)
                            | ImportedBindingTarget::OpaqueExport { .. }
                            | ImportedBindingTarget::ExternalExport { .. }
                            | ImportedBindingTarget::Unresolved,
                        )
                        | None => solver.seed(TargetNode::Binding(binding), TargetState::unknown()),
                    },
                    BindingKind::Parameter | BindingKind::Catch => {
                        solver.seed(TargetNode::Binding(binding), TargetState::unknown());
                    }
                    BindingKind::Const
                    | BindingKind::Let
                    | BindingKind::Class
                    | BindingKind::Var
                    | BindingKind::Function => {}
                }
            }

            for (function, function_ir) in module_ir.functions() {
                let function = ProgramFunctionId::new(module, function);

                if let Some(binding) = function_ir.self_binding() {
                    solver.seed(
                        TargetNode::Binding(ProgramBindingId::new(module, binding)),
                        TargetState::exact(function),
                    );
                }

                solver.add_function(module_ir, function, function_ir);
            }
        }

        solver.solve()
    }

    pub(super) fn target_set(&self, function: ProgramFunctionId, value: ValueId) -> CallTargetSet {
        self.state(TargetNode::Value(ProgramValue::new(function, value)))
            .as_target_set()
    }

    pub(super) fn binding_targets(&self, binding: ProgramBindingId) -> Box<[ProgramFunctionId]> {
        match self.state(TargetNode::Binding(binding)) {
            TargetState::Bottom => Box::new([]),
            TargetState::Known { functions, .. } => functions.into_iter().collect(),
        }
    }

    pub(super) fn is_forwarding_use(&self, use_site: (ProgramOperationId, u32)) -> bool {
        self.forwarding_uses.contains(&use_site)
    }

    pub(super) fn direct_eval_references(&self) -> Vec<FunctionReference> {
        let mut references = Vec::new();

        for exposure in &self.direct_eval {
            for binding in &exposure.bindings {
                references.extend(self.binding_targets(*binding).iter().map(|target| {
                    FunctionReference::new(
                        *target,
                        FunctionReferenceSite::DirectEval {
                            operation: exposure.operation,
                            binding: *binding,
                        },
                    )
                }));
            }
        }

        references
    }

    fn state(&self, node: TargetNode) -> TargetState {
        self.states
            .get(&node)
            .cloned()
            .unwrap_or_else(TargetState::bottom)
    }
}

struct TargetSolver {
    states: FxHashMap<TargetNode, TargetState>,
    dependents: FxHashMap<TargetNode, Vec<TargetNode>>,
    forwarding_uses: FxHashSet<(ProgramOperationId, u32)>,
    direct_eval: Vec<DirectEvalExposure>,
    work: WorkQueue<TargetNode>,
}

impl TargetSolver {
    fn new() -> Self {
        Self {
            states: FxHashMap::default(),
            dependents: FxHashMap::default(),
            forwarding_uses: FxHashSet::default(),
            direct_eval: Vec::new(),
            work: WorkQueue::new(),
        }
    }

    fn add_function(
        &mut self,
        module: &evrel_js_ir::JsModuleIr,
        function: ProgramFunctionId,
        function_ir: &JsFunctionIr,
    ) {
        for (value, data) in function_ir.values() {
            let value_node = TargetNode::Value(ProgramValue::new(function, value));
            match data.definition() {
                ValueDefinition::FunctionParameter { .. } => {
                    self.seed(value_node, TargetState::unknown());
                }
                ValueDefinition::OperationResult {
                    operation,
                    result_index: 0,
                } => match function_ir.operation(*operation).map(|data| data.kind()) {
                    Some(OperationKind::CreateFunction(create)) => self.seed(
                        value_node,
                        TargetState::exact(ProgramFunctionId::new(
                            function.module(),
                            create.function(),
                        )),
                    ),
                    Some(OperationKind::LoadBinding(_)) => {}
                    _ => self.seed(value_node, TargetState::unknown()),
                },
                ValueDefinition::OperationResult { .. } => {
                    self.seed(value_node, TargetState::unknown());
                }
                ValueDefinition::BlockParameter {
                    block,
                    parameter_index,
                } => {
                    let source = function_ir
                        .block(*block)
                        .and_then(|block| block.parameters().get(*parameter_index as usize))
                        .map(|parameter| parameter.source());
                    if source != Some(BlockParameterSource::Forwarded) {
                        self.seed(value_node, TargetState::unknown());
                    }
                }
            }
        }

        for (operation_id, operation) in function_ir.operations() {
            let program_operation = ProgramOperationId::new(function, operation_id);

            match operation.kind() {
                OperationKind::InitializeBinding(initialize) => {
                    if let Some(source) = operation.operands().first() {
                        self.add_value_binding_dependency(
                            function,
                            *source,
                            ProgramBindingId::new(function.module(), initialize.binding()),
                            program_operation,
                            0,
                        );
                    }
                }
                OperationKind::StoreBinding(store) => {
                    if let Some(source) = operation.operands().first() {
                        self.add_value_binding_dependency(
                            function,
                            *source,
                            ProgramBindingId::new(function.module(), store.binding()),
                            program_operation,
                            0,
                        );
                    }
                }
                OperationKind::LoadBinding(load) => {
                    if let Some(result) = operation.results().first() {
                        self.add_dependency(
                            TargetNode::Binding(ProgramBindingId::new(
                                function.module(),
                                load.binding(),
                            )),
                            TargetNode::Value(ProgramValue::new(function, *result)),
                        );
                    }
                }
                OperationKind::DestructureBinding(destructure) => {
                    for binding in destructure.pattern().binding_ids() {
                        self.seed(
                            TargetNode::Binding(ProgramBindingId::new(function.module(), binding)),
                            TargetState::unknown(),
                        );
                    }
                }
                OperationKind::DestructureAssignment(destructure) => {
                    for binding in destructure.pattern().binding_ids() {
                        self.seed(
                            TargetNode::Binding(ProgramBindingId::new(function.module(), binding)),
                            TargetState::unknown(),
                        );
                    }
                }
                _ => {}
            }

            for successor in operation.successors() {
                let produced = successor.produced_argument_count();
                let arguments = successor.arguments(operation.operands());
                let range = successor.argument_operand_range();
                let Some(block) = function_ir.block(successor.target().block()) else {
                    continue;
                };

                for (offset, argument) in arguments.iter().copied().enumerate() {
                    let Some(parameter) = block.parameters().get(produced + offset) else {
                        continue;
                    };
                    if parameter.source() != BlockParameterSource::Forwarded {
                        continue;
                    }

                    self.add_dependency(
                        TargetNode::Value(ProgramValue::new(function, argument)),
                        TargetNode::Value(ProgramValue::new(function, parameter.value())),
                    );
                    self.forwarding_uses.insert((
                        program_operation,
                        u32::try_from(range.start + offset).expect("operand index must fit in u32"),
                    ));
                }
            }

            if is_direct_eval_call(module, function_ir, operation_id) {
                let bindings = visible_bindings(module, function)
                    .into_iter()
                    .map(|binding| ProgramBindingId::new(function.module(), binding))
                    .collect::<Box<[_]>>();
                for binding in &bindings {
                    self.seed(TargetNode::Binding(*binding), TargetState::unknown());
                }
                self.direct_eval.push(DirectEvalExposure {
                    operation: program_operation,
                    bindings,
                });
            }
        }
    }

    fn add_value_binding_dependency(
        &mut self,
        function: ProgramFunctionId,
        value: ValueId,
        binding: ProgramBindingId,
        operation: ProgramOperationId,
        operand_index: u32,
    ) {
        self.add_dependency(
            TargetNode::Value(ProgramValue::new(function, value)),
            TargetNode::Binding(binding),
        );
        self.forwarding_uses.insert((operation, operand_index));
    }

    fn add_dependency(&mut self, source: TargetNode, target: TargetNode) {
        self.dependents.entry(source).or_default().push(target);
    }

    fn seed(&mut self, node: TargetNode, state: TargetState) {
        if self
            .states
            .entry(node)
            .or_insert_with(TargetState::bottom)
            .join(&state)
        {
            self.work.push(node);
        }
    }

    fn solve(mut self) -> ProgramFunctionTargets {
        while let Some(source) = self.work.pop() {
            let state = self
                .states
                .get(&source)
                .cloned()
                .unwrap_or_else(TargetState::bottom);
            let dependents = self
                .dependents
                .get(&source)
                .into_iter()
                .flatten()
                .copied()
                .collect::<Vec<_>>();

            for dependent in dependents {
                if self
                    .states
                    .entry(dependent)
                    .or_insert_with(TargetState::bottom)
                    .join(&state)
                {
                    self.work.push(dependent);
                }
            }
        }

        ProgramFunctionTargets {
            states: self.states,
            forwarding_uses: self.forwarding_uses,
            direct_eval: self.direct_eval.into_boxed_slice(),
        }
    }
}

fn visible_bindings(
    module: &evrel_js_ir::JsModuleIr,
    function: ProgramFunctionId,
) -> Vec<evrel_js_ir::BindingId> {
    let mut scopes = FxHashSet::default();
    let mut current = Some(function.function());

    while let Some(scope) = current {
        scopes.insert(scope);
        current = module
            .function(scope)
            .and_then(|function| function.parent_function());
    }

    module
        .bindings()
        .filter_map(|(binding, data)| {
            scopes
                .contains(&data.declaring_function())
                .then_some(binding)
        })
        .collect()
}
