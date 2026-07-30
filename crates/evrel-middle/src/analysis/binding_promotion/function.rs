//! Function-local binding-promotion eligibility.

use std::collections::BTreeMap;

use evrel_ir::{
    BindingId, FunctionIr, FunctionMode, OperationId, OperationKind, ValueDefinition, ValueId,
};

use super::super::{RegionControlFlowGraph, RegionDominatorTree};

/// Operations belonging to one binding that can be replaced by SSA values.
#[derive(Debug, Clone)]
pub struct PromotableBinding {
    initialization: OperationId,
    stores: Vec<OperationId>,
    loads: Vec<OperationId>,
}

impl PromotableBinding {
    /// Returns the binding's unique initialization operation.
    pub const fn initialization(&self) -> OperationId {
        self.initialization
    }

    /// Returns assignments to the initialized binding.
    pub fn stores(&self) -> &[OperationId] {
        &self.stores
    }

    /// Returns reads of the binding.
    pub fn loads(&self) -> &[OperationId] {
        &self.loads
    }

    /// Iterates over every operation removed by promotion.
    ///
    /// The order is deterministic but does not represent program order.
    /// Operation removal does not depend on ordering.
    pub fn operations(&self) -> impl Iterator<Item = OperationId> + '_ {
        std::iter::once(self.initialization)
            .chain(self.stores.iter().copied())
            .chain(self.loads.iter().copied())
    }
}

/// Promotable bindings within one function.
#[derive(Debug, Clone, Default)]
pub struct FunctionBindingPromotion {
    bindings: BTreeMap<BindingId, PromotableBinding>,
}

impl FunctionBindingPromotion {
    /// Returns whether this function has no promotable bindings.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Returns whether one binding is promotable.
    pub fn is_promotable(&self, binding: BindingId) -> bool {
        self.bindings.contains_key(&binding)
    }

    /// Iterates over promotable bindings in deterministic binding-ID order.
    pub fn promotable_bindings(&self) -> impl Iterator<Item = BindingId> + '_ {
        self.bindings.keys().copied()
    }

    /// Returns the promotion data for one eligible binding.
    pub fn promotable_binding(&self, binding: BindingId) -> Option<&PromotableBinding> {
        self.bindings.get(&binding)
    }
}

/// Incrementally collects function-local promotion candidates.
///
/// Module-level analysis chooses candidate binding kinds and rejects bindings
/// affected by exports, captures, or direct eval. This builder records local
/// operation shapes and validates their control flow.
#[derive(Debug, Default)]
pub(super) struct FunctionBindingPromotionBuilder {
    candidates: BTreeMap<BindingId, BindingCandidate>,
}

impl FunctionBindingPromotionBuilder {
    /// Adds a module-approved candidate binding.
    pub(super) fn add_candidate(&mut self, binding: BindingId) {
        assert!(
            self.candidates
                .insert(binding, BindingCandidate::new())
                .is_none(),
            "binding promotion candidate was added twice",
        );
    }

    /// Rejects a candidate because of information discovered outside its
    /// declaring function.
    pub(super) fn reject(&mut self, binding: BindingId) {
        if let Some(candidate) = self.candidates.get_mut(&binding) {
            candidate.reject();
        }
    }

    /// Rejects every candidate declared by this function.
    pub(super) fn reject_all(&mut self) {
        for candidate in self.candidates.values_mut() {
            candidate.reject();
        }
    }

    /// Records one operation that references a candidate binding.
    ///
    /// References from other functions are rejected by module analysis before
    /// this method is called. This method validates only local placement and
    /// operation shape.
    pub(super) fn record_reference(
        &mut self,
        function: &FunctionIr,
        operation: OperationId,
        binding: BindingId,
    ) {
        let Some(candidate) = self.candidates.get_mut(&binding) else {
            return;
        };

        let operation_data = function
            .operation(operation)
            .expect("binding reference operation must remain live");

        if function.block_region(operation_data.block()) != Some(function.body_region()) {
            candidate.reject();
            return;
        }

        match operation_data.kind() {
            OperationKind::InitializeBinding(initialize) if initialize.binding() == binding => {
                candidate.initialization = Some(operation);
            }

            OperationKind::StoreBinding(store) if store.binding() == binding => {
                candidate.stores.push(operation);
            }

            OperationKind::LoadBinding(load) if load.binding() == binding => {
                candidate.loads.push(operation);
            }

            // Destructuring, per-iteration binding metadata, and future
            // operations with specialized binding semantics stay explicit.
            _ => candidate.reject(),
        }
    }

    /// Finishes local eligibility using ordinary regional control flow.
    pub(super) fn finish(self, function: &FunctionIr) -> FunctionBindingPromotion {
        // Suspension is not yet represented as explicit regional CFG edges.
        if function.mode() != FunctionMode::Normal {
            return FunctionBindingPromotion::default();
        }

        let Ok(graph) = RegionControlFlowGraph::compute(function, function.body_region()) else {
            // Locally handled exceptional transfer must be explicit before
            // promotion can reason about every path.
            return FunctionBindingPromotion::default();
        };

        let dominance = RegionDominatorTree::compute(&graph);

        let bindings = self
            .candidates
            .into_iter()
            .filter_map(|(binding, candidate)| {
                finish_candidate(function, &graph, &dominance, candidate)
                    .map(|promotion| (binding, promotion))
            })
            .collect();

        FunctionBindingPromotion { bindings }
    }
}

#[derive(Debug)]
struct BindingCandidate {
    initialization: Option<OperationId>,
    stores: Vec<OperationId>,
    loads: Vec<OperationId>,
    valid: bool,
}

impl BindingCandidate {
    fn new() -> Self {
        Self {
            initialization: None,
            stores: Vec::new(),
            loads: Vec::new(),
            valid: true,
        }
    }

    fn reject(&mut self) {
        self.valid = false;
    }
}

fn finish_candidate(
    function: &FunctionIr,
    graph: &RegionControlFlowGraph,
    dominance: &RegionDominatorTree,
    candidate: BindingCandidate,
) -> Option<PromotableBinding> {
    if !candidate.valid {
        return None;
    }

    let initialization = candidate.initialization?;
    let initialization_data = function.operation(initialization)?;
    let initialization_block = initialization_data.block();

    if !graph.is_reachable(initialization_block) {
        return None;
    }

    // Initialization must execute before every load and store. This prevents
    // promotion from turning a runtime initialization error into an ordinary
    // SSA definition.
    for operation in candidate.stores.iter().chain(&candidate.loads).copied() {
        let operation_data = function.operation(operation)?;

        if !graph.is_reachable(operation_data.block())
            || !operation_dominates(function, dominance, initialization, operation)
        {
            return None;
        }
    }

    // Removing the binding write must not erase ECMAScript named evaluation
    // for an anonymous function or class assigned to the binding.
    for operation in std::iter::once(initialization).chain(candidate.stores.iter().copied()) {
        let value = *function.operation(operation)?.operands().first()?;

        if requires_named_evaluation(function, value) {
            return None;
        }
    }

    Some(PromotableBinding {
        initialization,
        stores: candidate.stores,
        loads: candidate.loads,
    })
}

fn operation_dominates(
    function: &FunctionIr,
    dominance: &RegionDominatorTree,
    dominator: OperationId,
    operation: OperationId,
) -> bool {
    let dominator_block = function
        .operation(dominator)
        .expect("dominator operation must remain live")
        .block();
    let operation_block = function
        .operation(operation)
        .expect("dominated operation must remain live")
        .block();

    if dominator_block != operation_block {
        return dominance.dominates(dominator_block, operation_block);
    }

    let block = function
        .block(dominator_block)
        .expect("operation block must remain live");

    let dominator_index = block
        .operations()
        .iter()
        .position(|&candidate| candidate == dominator)
        .expect("block must contain the dominator operation");
    let operation_index = block
        .operations()
        .iter()
        .position(|&candidate| candidate == operation)
        .expect("block must contain the dominated operation");

    dominator_index < operation_index
}

fn requires_named_evaluation(function: &FunctionIr, value: ValueId) -> bool {
    let value = function
        .value(value)
        .expect("binding write must use a live value");

    let ValueDefinition::OperationResult { operation, .. } = value.definition() else {
        return false;
    };

    matches!(
        function
            .operation(*operation)
            .expect("value definition must remain live")
            .kind(),
        OperationKind::CreateFunction(_) | OperationKind::CreateClass(_)
    )
}

#[cfg(test)]
mod tests {
    use evrel_ir::{
        BindingKind, BlockTarget, ConstantOp, ConstantValue, CreateFunctionOp, FunctionKind,
        FunctionMode, IfOp, InitializeBindingOp, JumpOp, LoadBindingOp, ModuleBuilder, ModuleIr,
        OperationKind, StoreBindingOp, UnwindTarget,
    };

    use super::FunctionBindingPromotionBuilder;

    #[test]
    fn accepts_a_dominated_initialization_store_and_load() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (binding, initialization, store, load) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let binding = module_builder.create_binding(function_id, "value", BindingKind::Var);
            let mut builder = module_builder.function_builder(function_id);

            let initial = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
                [],
                UnwindTarget::Propagate,
            );
            let initial = builder.operation_results(initial)[0];
            let initialization = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::InitializeBinding(InitializeBindingOp::new(binding)),
                [initial],
                UnwindTarget::Propagate,
            );

            let replacement = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(1.0))),
                [],
                UnwindTarget::Propagate,
            );
            let replacement = builder.operation_results(replacement)[0];
            let store = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::StoreBinding(StoreBindingOp::new(binding)),
                [replacement],
                UnwindTarget::Propagate,
            );
            let load = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::LoadBinding(LoadBindingOp::new(binding)),
                [],
                UnwindTarget::Propagate,
            );

            (binding, initialization, store, load)
        };

        let function = module.function(function_id).unwrap();
        let mut builder = FunctionBindingPromotionBuilder::default();
        builder.add_candidate(binding);
        builder.record_reference(function, initialization, binding);
        builder.record_reference(function, store, binding);
        builder.record_reference(function, load, binding);

        let promotion = builder.finish(function);
        let binding_promotion = promotion.promotable_binding(binding).unwrap();

        assert_eq!(binding_promotion.initialization(), initialization);
        assert_eq!(binding_promotion.stores(), [store]);
        assert_eq!(binding_promotion.loads(), [load]);
    }

    #[test]
    fn rejects_an_initialization_that_does_not_dominate_a_load() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (binding, initialization, load) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let binding = module_builder.create_binding(function_id, "value", BindingKind::Var);
            let mut builder = module_builder.function_builder(function_id);
            let left = builder.create_block();
            let right = builder.create_block();
            let join = builder.create_block();

            let condition = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Boolean(true))),
                [],
                UnwindTarget::Propagate,
            );
            let condition = builder.operation_results(condition)[0];
            builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::If(IfOp::new(
                    BlockTarget::new(left, 0),
                    BlockTarget::new(right, 0),
                    join,
                )),
                [condition],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(left);
            let initial = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
                [],
                UnwindTarget::Propagate,
            );
            let initial = builder.operation_results(initial)[0];
            let initialization = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::InitializeBinding(InitializeBindingOp::new(binding)),
                [initial],
                UnwindTarget::Propagate,
            );
            builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(join, 0))),
                [],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(right);
            builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(join, 0))),
                [],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(join);
            let load = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::LoadBinding(LoadBindingOp::new(binding)),
                [],
                UnwindTarget::Propagate,
            );

            (binding, initialization, load)
        };

        let function = module.function(function_id).unwrap();
        let mut builder = FunctionBindingPromotionBuilder::default();
        builder.add_candidate(binding);
        builder.record_reference(function, initialization, binding);
        builder.record_reference(function, load, binding);

        let promotion = builder.finish(function);

        assert!(!promotion.is_promotable(binding));
    }

    #[test]
    fn rejects_a_write_that_requires_named_evaluation() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (binding, initialization, store) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let binding = module_builder.create_binding(function_id, "value", BindingKind::Var);
            let created_function = module_builder.create_function(
                FunctionKind::Ordinary,
                FunctionMode::Normal,
                function_id,
            );
            let mut builder = module_builder.function_builder(function_id);

            let initial = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
                [],
                UnwindTarget::Propagate,
            );
            let initial = builder.operation_results(initial)[0];
            let initialization = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::InitializeBinding(InitializeBindingOp::new(binding)),
                [initial],
                UnwindTarget::Propagate,
            );

            let function = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::CreateFunction(CreateFunctionOp::new(created_function)),
                [],
                UnwindTarget::Propagate,
            );
            let function = builder.operation_results(function)[0];
            let store = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::StoreBinding(StoreBindingOp::new(binding)),
                [function],
                UnwindTarget::Propagate,
            );

            (binding, initialization, store)
        };

        let function = module.function(function_id).unwrap();
        let mut builder = FunctionBindingPromotionBuilder::default();
        builder.add_candidate(binding);
        builder.record_reference(function, initialization, binding);
        builder.record_reference(function, store, binding);

        let promotion = builder.finish(function);

        assert!(!promotion.is_promotable(binding));
    }
}
