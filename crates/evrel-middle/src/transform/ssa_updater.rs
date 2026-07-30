//! Targeted SSA construction for one region-local value.

use evrel_ir::{BlockId, FunctionEditor, FunctionIr, OperationId, ValueId};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::analysis::{RegionControlFlowGraph, RegionDominanceFrontier, RegionDominatorTree};

/// Uses and an optional definition occurring at one operation position.
///
/// Uses occur before the definition. This matches operations such as a binding
/// store whose operand is evaluated before the stored value becomes current.
#[derive(Debug, Default)]
struct SsaEvents {
    uses: Vec<ValueId>,
    definition: Option<ValueId>,
}

/// A value available while constructing the updated SSA web.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReachingValue {
    /// An SSA value already present in the IR.
    Existing(ValueId),

    /// The merge parameter that will be inserted at a block.
    Merge(BlockId),
}

/// One operation result that should be replaced.
#[derive(Debug, Clone, Copy)]
pub(super) struct UseRewrite {
    result: ValueId,
    replacement: ReachingValue,
}

impl UseRewrite {
    /// Returns the operation result being replaced.
    pub(super) const fn result(self) -> ValueId {
        self.result
    }

    /// Returns the value that should replace the result.
    pub(super) const fn replacement(self) -> ReachingValue {
        self.replacement
    }
}

/// Builds an SSA update for one caller-identified value web.
///
/// The caller registers definitions and uses explicitly. The updater decides
/// where merge parameters are required and which value reaches each use. It
/// does not discover bindings, decide promotability, or mutate the IR.
pub(super) struct SsaUpdater<'ir> {
    function: &'ir FunctionIr,
    graph: &'ir RegionControlFlowGraph,
    dominance: &'ir RegionDominatorTree,
    frontier: &'ir RegionDominanceFrontier,
    entry_value: Option<ValueId>,
    events: FxHashMap<OperationId, SsaEvents>,
}

impl<'ir> SsaUpdater<'ir> {
    /// Creates an updater over one already-analyzed region.
    pub(super) fn new(
        function: &'ir FunctionIr,
        graph: &'ir RegionControlFlowGraph,
        dominance: &'ir RegionDominatorTree,
        frontier: &'ir RegionDominanceFrontier,
        entry_value: Option<ValueId>,
    ) -> Self {
        if let Some(value) = entry_value {
            assert!(
                function.value(value).is_some(),
                "SSA entry value must belong to the function",
            );
        }

        Self {
            function,
            graph,
            dominance,
            frontier,
            entry_value,
            events: FxHashMap::default(),
        }
    }

    /// Records a value that becomes current after the operation executes.
    pub(super) fn add_definition(&mut self, operation: OperationId, value: ValueId) {
        self.validate_event(operation, value);

        assert!(
            self.events
                .entry(operation)
                .or_default()
                .definition
                .replace(value)
                .is_none(),
            "an operation may define the updated value only once",
        );
    }

    /// Records an operation result that reads the current value.
    pub(super) fn add_use(&mut self, operation: OperationId, result: ValueId) {
        self.validate_event(operation, result);

        self.events.entry(operation).or_default().uses.push(result);
    }

    /// Computes merge placement and reaching values without mutating the IR.
    ///
    /// Returns `None` when a reachable use or incoming merge edge has no
    /// reaching definition.
    pub(super) fn finish(self) -> Option<SsaUpdate> {
        let definition_blocks = self.definition_blocks_in_reverse_postorder();

        let merge_blocks = self.frontier.iterated_frontier(definition_blocks);

        let merge_set = merge_blocks.iter().copied().collect::<FxHashSet<_>>();

        let mut outgoing_values = FxHashMap::default();
        let mut use_rewrites = Vec::new();

        let entry_value = self.entry_value.map(ReachingValue::Existing);

        let mut stack = vec![(self.graph.entry(), entry_value)];

        while let Some((block, inherited)) = stack.pop() {
            let mut current = if merge_set.contains(&block) {
                Some(ReachingValue::Merge(block))
            } else {
                inherited
            };

            let block_data = self
                .function
                .block(block)
                .expect("dominator-tree block must remain live");

            for operation in block_data
                .operations()
                .iter()
                .copied()
                .chain(block_data.terminator())
            {
                let Some(events) = self.events.get(&operation) else {
                    continue;
                };

                for &result in &events.uses {
                    use_rewrites.push(UseRewrite {
                        result,
                        replacement: current?,
                    });
                }

                if let Some(definition) = events.definition {
                    current = Some(ReachingValue::Existing(definition));
                }
            }

            if let Some(value) = current {
                outgoing_values.insert(block, value);
            }

            for &child in self.dominance.children(block)?.iter().rev() {
                stack.push((child, current));
            }
        }

        // Every reachable edge entering a merge must carry a real reaching
        // value. Unreachable predecessors are handled later by the IR editor
        // because they cannot execute.
        for &merge in &merge_blocks {
            for &edge in self.graph.predecessor_edges(merge)? {
                let predecessor = self.graph.edge(edge)?.source();

                if self.graph.is_reachable(predecessor)
                    && !outgoing_values.contains_key(&predecessor)
                {
                    return None;
                }
            }
        }

        Some(SsaUpdate {
            merge_blocks,
            outgoing_values,
            use_rewrites,
        })
    }

    fn definition_blocks_in_reverse_postorder(&self) -> Vec<BlockId> {
        self.graph
            .reverse_postorder()
            .iter()
            .copied()
            .filter(|&block| {
                let block = self
                    .function
                    .block(block)
                    .expect("CFG block must remain live");

                block
                    .operations()
                    .iter()
                    .copied()
                    .chain(block.terminator())
                    .any(|operation| {
                        self.events
                            .get(&operation)
                            .is_some_and(|events| events.definition.is_some())
                    })
            })
            .collect()
    }

    fn validate_event(&self, operation: OperationId, value: ValueId) {
        let operation_data = self
            .function
            .operation(operation)
            .expect("SSA event operation must belong to the function");

        assert_eq!(
            self.function.block_region(operation_data.block()),
            Some(self.graph.region()),
            "SSA events must belong to the updater region",
        );

        assert!(
            self.graph.is_reachable(operation_data.block()),
            "SSA events must belong to reachable blocks",
        );

        assert!(
            self.function.value(value).is_some(),
            "SSA event value must belong to the function",
        );
    }
}

/// An immutable SSA rewrite plan.
///
/// Merge blocks refer to parameters that do not exist yet. `ReachingValue::Merge`
/// is resolved after those parameters are inserted by the eventual IR editor.
#[derive(Debug)]
pub(super) struct SsaUpdate {
    merge_blocks: Vec<BlockId>,
    outgoing_values: FxHashMap<BlockId, ReachingValue>,
    use_rewrites: Vec<UseRewrite>,
}

impl SsaUpdate {
    /// Applies this update while preserving function IR invariants.
    ///
    /// All merge parameters are allocated before incoming arguments are
    /// requested. This allows cyclic merge blocks to forward newly allocated
    /// parameters through one another.
    pub(super) fn apply(self, editor: &mut FunctionEditor<'_>) {
        let Self {
            merge_blocks,
            outgoing_values,
            use_rewrites,
        } = self;

        let parameters = editor.append_forwarded_block_parameters(
            merge_blocks,
            |parameters, target, predecessor, _terminator, _successor_index| {
                let reaching = outgoing_values
                    .get(&predecessor)
                    .copied()
                    .unwrap_or(ReachingValue::Merge(target));

                resolve_reaching_value(reaching, parameters)
            },
        );

        // Process replacements in reverse discovery order. A replacement may
        // itself be the result of an earlier registered use, so reversing
        // transitively collapses such chains before their operations are
        // eventually removed by binding promotion.
        for rewrite in use_rewrites.into_iter().rev() {
            let replacement = resolve_reaching_value(rewrite.replacement, &parameters);

            editor.replace_all_uses(rewrite.result, replacement);
        }
    }

    #[cfg(test)]
    pub(super) fn merge_blocks(&self) -> &[BlockId] {
        &self.merge_blocks
    }

    #[cfg(test)]
    pub(super) fn outgoing_value(&self, block: BlockId) -> Option<ReachingValue> {
        self.outgoing_values.get(&block).copied()
    }

    #[cfg(test)]
    pub(super) fn use_rewrites(&self) -> &[UseRewrite] {
        &self.use_rewrites
    }
}

fn resolve_reaching_value(reaching: ReachingValue, parameters: &[(BlockId, ValueId)]) -> ValueId {
    match reaching {
        ReachingValue::Existing(value) => value,

        ReachingValue::Merge(block) => parameters
            .iter()
            .find_map(|&(candidate, parameter)| (candidate == block).then_some(parameter))
            .expect("merge value must reference an allocated parameter"),
    }
}

#[cfg(test)]
mod tests {
    use evrel_ir::{
        BinaryOp, BinaryOperator, BlockParameterSource, BlockTarget, ConstantOp, ConstantValue,
        FunctionEditor, IfOp, JumpOp, ModuleBuilder, ModuleIr, OperationKind, UnwindTarget,
    };

    use super::{ReachingValue, SsaUpdater};
    use crate::analysis::{RegionControlFlowGraph, RegionDominanceFrontier, RegionDominatorTree};

    #[test]
    fn carries_a_definition_to_a_later_use() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (definition_operation, definition, use_operation, used_result) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);

            let definition_operation = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(1.0))),
                [],
                UnwindTarget::Propagate,
            );
            let definition = builder.operation_results(definition_operation)[0];
            let use_operation = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(2.0))),
                [],
                UnwindTarget::Propagate,
            );
            let used_result = builder.operation_results(use_operation)[0];

            (definition_operation, definition, use_operation, used_result)
        };

        let function = module.function(function_id).unwrap();
        let graph = RegionControlFlowGraph::compute(function, function.body_region()).unwrap();
        let dominance = RegionDominatorTree::compute(&graph);
        let frontier = RegionDominanceFrontier::compute(&graph, &dominance);
        let mut updater = SsaUpdater::new(function, &graph, &dominance, &frontier, None);

        updater.add_definition(definition_operation, definition);
        updater.add_use(use_operation, used_result);

        let update = updater.finish().unwrap();

        assert!(update.merge_blocks().is_empty());
        assert_eq!(
            update.outgoing_value(function.entry_block()),
            Some(ReachingValue::Existing(definition)),
        );
        assert_eq!(update.use_rewrites().len(), 1);
        assert_eq!(update.use_rewrites()[0].result(), used_result);
        assert_eq!(
            update.use_rewrites()[0].replacement(),
            ReachingValue::Existing(definition),
        );
    }

    #[test]
    fn places_a_merge_for_definitions_on_both_diamond_paths() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (
            left,
            right,
            join,
            left_definition_operation,
            left_definition,
            right_definition_operation,
            right_definition,
            use_operation,
            used_result,
        ) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
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
            let left_definition_operation = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(1.0))),
                [],
                UnwindTarget::Propagate,
            );
            let left_definition = builder.operation_results(left_definition_operation)[0];
            builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(join, 0))),
                [],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(right);
            let right_definition_operation = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(2.0))),
                [],
                UnwindTarget::Propagate,
            );
            let right_definition = builder.operation_results(right_definition_operation)[0];
            builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(join, 0))),
                [],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(join);
            let use_operation = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
                [],
                UnwindTarget::Propagate,
            );
            let used_result = builder.operation_results(use_operation)[0];

            (
                left,
                right,
                join,
                left_definition_operation,
                left_definition,
                right_definition_operation,
                right_definition,
                use_operation,
                used_result,
            )
        };

        let function = module.function(function_id).unwrap();
        let graph = RegionControlFlowGraph::compute(function, function.body_region()).unwrap();
        let dominance = RegionDominatorTree::compute(&graph);
        let frontier = RegionDominanceFrontier::compute(&graph, &dominance);
        let mut updater = SsaUpdater::new(function, &graph, &dominance, &frontier, None);

        updater.add_definition(left_definition_operation, left_definition);
        updater.add_definition(right_definition_operation, right_definition);
        updater.add_use(use_operation, used_result);

        let update = updater.finish().unwrap();

        assert_eq!(update.merge_blocks(), [join]);
        assert_eq!(
            update.outgoing_value(left),
            Some(ReachingValue::Existing(left_definition)),
        );
        assert_eq!(
            update.outgoing_value(right),
            Some(ReachingValue::Existing(right_definition)),
        );
        assert_eq!(
            update.outgoing_value(join),
            Some(ReachingValue::Merge(join)),
        );
        assert_eq!(
            update.use_rewrites()[0].replacement(),
            ReachingValue::Merge(join),
        );
    }

    #[test]
    fn does_not_allow_a_later_definition_to_reach_an_earlier_use() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (use_operation, used_result, definition_operation, definition) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);

            let use_operation = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
                [],
                UnwindTarget::Propagate,
            );
            let used_result = builder.operation_results(use_operation)[0];
            let definition_operation = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(1.0))),
                [],
                UnwindTarget::Propagate,
            );
            let definition = builder.operation_results(definition_operation)[0];

            (use_operation, used_result, definition_operation, definition)
        };

        let function = module.function(function_id).unwrap();
        let graph = RegionControlFlowGraph::compute(function, function.body_region()).unwrap();
        let dominance = RegionDominatorTree::compute(&graph);
        let frontier = RegionDominanceFrontier::compute(&graph, &dominance);
        let mut updater = SsaUpdater::new(function, &graph, &dominance, &frontier, None);

        updater.add_use(use_operation, used_result);
        updater.add_definition(definition_operation, definition);

        assert!(updater.finish().is_none());
    }

    #[test]
    fn rejects_a_reachable_use_without_a_definition() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (use_operation, used_result) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let use_operation = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
                [],
                UnwindTarget::Propagate,
            );
            let used_result = builder.operation_results(use_operation)[0];

            (use_operation, used_result)
        };

        let function = module.function(function_id).unwrap();
        let graph = RegionControlFlowGraph::compute(function, function.body_region()).unwrap();
        let dominance = RegionDominatorTree::compute(&graph);
        let frontier = RegionDominanceFrontier::compute(&graph, &dominance);
        let mut updater = SsaUpdater::new(function, &graph, &dominance, &frontier, None);

        updater.add_use(use_operation, used_result);

        assert!(updater.finish().is_none());
    }

    #[test]
    fn places_a_merge_at_a_loop_header() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (
            entry,
            header,
            body,
            initial_operation,
            initial,
            use_operation,
            used_result,
            iteration_operation,
            iteration,
        ) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let entry = builder.current_block();
            let header = builder.create_block();
            let body = builder.create_block();
            let exit = builder.create_block();

            let initial_operation = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(0.0))),
                [],
                UnwindTarget::Propagate,
            );
            let initial = builder.operation_results(initial_operation)[0];
            builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(header, 0))),
                [],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(header);
            let use_operation = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
                [],
                UnwindTarget::Propagate,
            );
            let used_result = builder.operation_results(use_operation)[0];
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
                    BlockTarget::new(body, 0),
                    BlockTarget::new(exit, 0),
                    exit,
                )),
                [condition],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(body);
            let iteration_operation = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(1.0))),
                [],
                UnwindTarget::Propagate,
            );
            let iteration = builder.operation_results(iteration_operation)[0];
            builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(header, 0))),
                [],
                UnwindTarget::Propagate,
            );

            (
                entry,
                header,
                body,
                initial_operation,
                initial,
                use_operation,
                used_result,
                iteration_operation,
                iteration,
            )
        };

        let function = module.function(function_id).unwrap();
        let graph = RegionControlFlowGraph::compute(function, function.body_region()).unwrap();
        let dominance = RegionDominatorTree::compute(&graph);
        let frontier = RegionDominanceFrontier::compute(&graph, &dominance);
        let mut updater = SsaUpdater::new(function, &graph, &dominance, &frontier, None);

        updater.add_definition(initial_operation, initial);
        updater.add_use(use_operation, used_result);
        updater.add_definition(iteration_operation, iteration);

        let update = updater.finish().unwrap();

        assert_eq!(update.merge_blocks(), [header]);
        assert_eq!(
            update.outgoing_value(entry),
            Some(ReachingValue::Existing(initial)),
        );
        assert_eq!(
            update.outgoing_value(body),
            Some(ReachingValue::Existing(iteration)),
        );
        assert_eq!(
            update.use_rewrites()[0].replacement(),
            ReachingValue::Merge(header),
        );
    }

    #[test]
    fn applies_a_diamond_merge_to_the_function() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (
            left,
            right,
            join,
            left_definition_operation,
            left_definition,
            right_definition_operation,
            right_definition,
            use_operation,
            used_result,
            consumer,
            left_jump,
            right_jump,
        ) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
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
            let left_definition_operation = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(1.0))),
                [],
                UnwindTarget::Propagate,
            );
            let left_definition = builder.operation_results(left_definition_operation)[0];
            let left_jump = builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(join, 0))),
                [],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(right);
            let right_definition_operation = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(2.0))),
                [],
                UnwindTarget::Propagate,
            );
            let right_definition = builder.operation_results(right_definition_operation)[0];
            let right_jump = builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(join, 0))),
                [],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(join);
            let use_operation = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
                [],
                UnwindTarget::Propagate,
            );
            let used_result = builder.operation_results(use_operation)[0];
            let consumer = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Binary(BinaryOp::new(BinaryOperator::StrictEqual)),
                [used_result, used_result],
                UnwindTarget::Propagate,
            );

            (
                left,
                right,
                join,
                left_definition_operation,
                left_definition,
                right_definition_operation,
                right_definition,
                use_operation,
                used_result,
                consumer,
                left_jump,
                right_jump,
            )
        };

        let update = {
            let function = module.function(function_id).unwrap();
            let graph = RegionControlFlowGraph::compute(function, function.body_region()).unwrap();
            let dominance = RegionDominatorTree::compute(&graph);
            let frontier = RegionDominanceFrontier::compute(&graph, &dominance);
            let mut updater = SsaUpdater::new(function, &graph, &dominance, &frontier, None);

            updater.add_definition(left_definition_operation, left_definition);
            updater.add_definition(right_definition_operation, right_definition);
            updater.add_use(use_operation, used_result);

            updater.finish().unwrap()
        };

        let function = module.function_mut(function_id).unwrap();
        let mut editor = FunctionEditor::new(function);
        update.apply(&mut editor);

        let join_block = editor.ir().block(join).unwrap();
        assert_eq!(join_block.parameters().len(), 1);

        let parameter = join_block.parameters()[0];
        assert_eq!(parameter.source(), BlockParameterSource::Forwarded);
        let parameter = parameter.value();

        assert_eq!(
            editor.ir().operation(left_jump).unwrap().operands(),
            [left_definition],
        );
        assert_eq!(
            editor.ir().operation(right_jump).unwrap().operands(),
            [right_definition],
        );
        assert_eq!(
            editor.ir().operation(consumer).unwrap().operands(),
            [parameter, parameter],
        );
        assert!(editor.ir().value(used_result).unwrap().uses().is_empty());
        assert_eq!(editor.ir().value(left_definition).unwrap().uses().len(), 1,);
        assert_eq!(editor.ir().value(right_definition).unwrap().uses().len(), 1,);
        assert!(editor.ir().block(left).unwrap().parameters().is_empty());
        assert!(editor.ir().block(right).unwrap().parameters().is_empty());
    }

    #[test]
    fn applies_a_loop_header_merge_to_the_function() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (
            header,
            initial_operation,
            initial,
            entry_jump,
            use_operation,
            used_result,
            consumer,
            iteration_operation,
            iteration,
            backedge,
        ) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let header = builder.create_block();
            let body = builder.create_block();
            let exit = builder.create_block();

            let initial_operation = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(0.0))),
                [],
                UnwindTarget::Propagate,
            );
            let initial = builder.operation_results(initial_operation)[0];
            let entry_jump = builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(header, 0))),
                [],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(header);
            let use_operation = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
                [],
                UnwindTarget::Propagate,
            );
            let used_result = builder.operation_results(use_operation)[0];
            let consumer = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Binary(BinaryOp::new(BinaryOperator::StrictEqual)),
                [used_result, used_result],
                UnwindTarget::Propagate,
            );
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
                    BlockTarget::new(body, 0),
                    BlockTarget::new(exit, 0),
                    exit,
                )),
                [condition],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(body);
            let iteration_operation = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(1.0))),
                [],
                UnwindTarget::Propagate,
            );
            let iteration = builder.operation_results(iteration_operation)[0];
            let backedge = builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(header, 0))),
                [],
                UnwindTarget::Propagate,
            );

            (
                header,
                initial_operation,
                initial,
                entry_jump,
                use_operation,
                used_result,
                consumer,
                iteration_operation,
                iteration,
                backedge,
            )
        };

        let update = {
            let function = module.function(function_id).unwrap();
            let graph = RegionControlFlowGraph::compute(function, function.body_region()).unwrap();
            let dominance = RegionDominatorTree::compute(&graph);
            let frontier = RegionDominanceFrontier::compute(&graph, &dominance);
            let mut updater = SsaUpdater::new(function, &graph, &dominance, &frontier, None);

            updater.add_definition(initial_operation, initial);
            updater.add_use(use_operation, used_result);
            updater.add_definition(iteration_operation, iteration);

            updater.finish().unwrap()
        };

        let function = module.function_mut(function_id).unwrap();
        let mut editor = FunctionEditor::new(function);
        update.apply(&mut editor);

        let header_block = editor.ir().block(header).unwrap();
        assert_eq!(header_block.parameters().len(), 1);

        let parameter = header_block.parameters()[0];
        assert_eq!(parameter.source(), BlockParameterSource::Forwarded);
        let parameter = parameter.value();

        assert_eq!(
            editor.ir().operation(entry_jump).unwrap().operands(),
            [initial],
        );
        assert_eq!(
            editor.ir().operation(backedge).unwrap().operands(),
            [iteration],
        );
        assert_eq!(
            editor.ir().operation(consumer).unwrap().operands(),
            [parameter, parameter],
        );
        assert!(editor.ir().value(used_result).unwrap().uses().is_empty());

        let parameter_uses = editor.ir().value(parameter).unwrap().uses();
        assert_eq!(parameter_uses.len(), 2);
        assert_eq!(parameter_uses[0].operation(), consumer);
        assert_eq!(parameter_uses[1].operation(), consumer);
    }
}
