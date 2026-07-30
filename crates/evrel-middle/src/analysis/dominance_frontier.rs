//! Dominance-frontier analysis for one IR region.

use evrel_ir::BlockId;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::work_queue::WorkQueue;

use super::{RegionControlFlowGraph, RegionDominatorTree};

/// Blocks where separately dominated control-flow paths meet.
#[derive(Debug, Clone)]
pub struct RegionDominanceFrontier {
    frontiers: FxHashMap<BlockId, Vec<BlockId>>,
}

impl RegionDominanceFrontier {
    /// Computes dominance frontiers for reachable blocks in one region.
    pub fn compute(graph: &RegionControlFlowGraph, dominance: &RegionDominatorTree) -> Self {
        let mut frontiers = graph
            .reverse_postorder()
            .iter()
            .copied()
            .map(|block| (block, Vec::new()))
            .collect::<FxHashMap<_, _>>();

        for &block in graph.reverse_postorder() {
            let predecessors = graph
                .predecessor_edges(block)
                .expect("CFG block must have a predecessor list")
                .iter()
                .filter_map(|&edge| graph.edge(edge))
                .map(|edge| edge.source())
                .filter(|predecessor| graph.is_reachable(*predecessor))
                .collect::<Vec<_>>();

            if predecessors.len() < 2 {
                continue;
            }

            // The entry has no immediate dominator. Treating it as its own
            // dominator allows a backedge to the entry to be represented.
            let immediate_dominator = dominance.immediate_dominator(block).unwrap_or(block);

            for mut runner in predecessors {
                while runner != immediate_dominator {
                    let frontier = frontiers
                        .get_mut(&runner)
                        .expect("dominance runner must be reachable");

                    if !frontier.contains(&block) {
                        frontier.push(block);
                    }

                    runner = dominance
                        .immediate_dominator(runner)
                        .expect("dominance runner must reach the join's immediate dominator");
                }
            }
        }

        Self { frontiers }
    }

    /// Returns the dominance frontier of a reachable block.
    pub fn frontier(&self, block: BlockId) -> Option<&[BlockId]> {
        self.frontiers.get(&block).map(Vec::as_slice)
    }

    /// Computes the iterated dominance frontier of definition blocks.
    ///
    /// This gives the merge blocks required for ordinary, non-pruned SSA
    /// construction. Definitions should be supplied in deterministic order.
    pub fn iterated_frontier(
        &self,
        definitions: impl IntoIterator<Item = BlockId>,
    ) -> Vec<BlockId> {
        let definitions = definitions.into_iter().collect::<Vec<_>>();
        let definition_set = definitions.iter().copied().collect::<FxHashSet<_>>();

        let mut queue = WorkQueue::new();
        let mut inserted = FxHashSet::default();
        let mut result = Vec::new();

        for definition in definitions {
            assert!(
                self.frontiers.contains_key(&definition),
                "definition block must be reachable",
            );

            queue.push(definition);
        }

        while let Some(block) = queue.pop() {
            for &frontier in self
                .frontier(block)
                .expect("queued block must be reachable")
            {
                if !inserted.insert(frontier) {
                    continue;
                }

                result.push(frontier);

                if !definition_set.contains(&frontier) {
                    queue.push(frontier);
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use evrel_ir::{
        BlockTarget, ConstantOp, ConstantValue, IfOp, JumpOp, ModuleBuilder, ModuleIr,
        OperationKind, UnwindTarget,
    };

    use super::RegionDominanceFrontier;
    use crate::analysis::{RegionControlFlowGraph, RegionDominatorTree};

    #[test]
    fn computes_the_frontier_of_a_diamond() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (entry, left, right, join) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let entry = builder.current_block();
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

            (entry, left, right, join)
        };

        let function = module.function(function_id).unwrap();
        let graph = RegionControlFlowGraph::compute(function, function.body_region()).unwrap();
        let dominance = RegionDominatorTree::compute(&graph);
        let frontier = RegionDominanceFrontier::compute(&graph, &dominance);

        assert_eq!(frontier.frontier(entry), Some([].as_slice()));
        assert_eq!(frontier.frontier(left), Some([join].as_slice()));
        assert_eq!(frontier.frontier(right), Some([join].as_slice()));
        assert_eq!(frontier.frontier(join), Some([].as_slice()));
    }

    #[test]
    fn computes_the_frontier_of_a_loop_backedge() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (header, body, exit) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let header = builder.create_block();
            let body = builder.create_block();
            let exit = builder.create_block();

            builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(header, 0))),
                [],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(header);
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
            builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(header, 0))),
                [],
                UnwindTarget::Propagate,
            );

            (header, body, exit)
        };

        let function = module.function(function_id).unwrap();
        let graph = RegionControlFlowGraph::compute(function, function.body_region()).unwrap();
        let dominance = RegionDominatorTree::compute(&graph);
        let frontier = RegionDominanceFrontier::compute(&graph, &dominance);

        assert_eq!(frontier.frontier(header), Some([header].as_slice()));
        assert_eq!(frontier.frontier(body), Some([header].as_slice()));
        assert_eq!(frontier.frontier(exit), Some([].as_slice()));
    }

    #[test]
    fn computes_an_iterated_frontier_across_two_merges() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (left, right, first_join, second_join) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let first_split = builder.create_block();
            let bypass = builder.create_block();
            let left = builder.create_block();
            let right = builder.create_block();
            let first_join = builder.create_block();
            let second_join = builder.create_block();

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
                    BlockTarget::new(first_split, 0),
                    BlockTarget::new(bypass, 0),
                    second_join,
                )),
                [condition],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(first_split);
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
                    first_join,
                )),
                [condition],
                UnwindTarget::Propagate,
            );

            for block in [left, right] {
                builder.switch_to_block(block);
                builder.terminate(
                    evrel_ir::LocationId::UNKNOWN,
                    OperationKind::Jump(JumpOp::new(BlockTarget::new(first_join, 0))),
                    [],
                    UnwindTarget::Propagate,
                );
            }

            builder.switch_to_block(first_join);
            builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(second_join, 0))),
                [],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(bypass);
            builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(second_join, 0))),
                [],
                UnwindTarget::Propagate,
            );

            (left, right, first_join, second_join)
        };

        let function = module.function(function_id).unwrap();
        let graph = RegionControlFlowGraph::compute(function, function.body_region()).unwrap();
        let dominance = RegionDominatorTree::compute(&graph);
        let frontier = RegionDominanceFrontier::compute(&graph, &dominance);

        assert_eq!(
            frontier.iterated_frontier([left, right]),
            [first_join, second_join],
        );
    }
}
