//! Dominance analysis for one IR region.

use evrel_ir::BlockId;
use rustc_hash::FxHashMap;

use super::RegionControlFlowGraph;

/// Dominance relationships among reachable blocks in one region.
#[derive(Debug, Clone)]
pub struct RegionDominatorTree {
    entry: BlockId,
    immediate_dominators: FxHashMap<BlockId, BlockId>,
    children: FxHashMap<BlockId, Vec<BlockId>>,
}

impl RegionDominatorTree {
    /// Computes dominance over the reachable portion of a reachable CFG.
    pub fn compute(graph: &RegionControlFlowGraph) -> Self {
        let reverse_postorder = graph.reverse_postorder();
        let entry = graph.entry();

        let order = reverse_postorder
            .iter()
            .copied()
            .enumerate()
            .map(|(index, block)| (block, index))
            .collect::<FxHashMap<_, _>>();

        let mut immediate_dominators = FxHashMap::default();
        immediate_dominators.insert(entry, entry);

        loop {
            let mut changed = false;

            for &block in reverse_postorder.iter().skip(1) {
                let predecessors = graph
                    .predecessor_edges(block)
                    .expect("CFG block must have a predecessor list")
                    .iter()
                    .filter_map(|&edge| graph.edge(edge))
                    .map(|edge| edge.source())
                    .filter(|predecessor| graph.is_reachable(*predecessor))
                    .collect::<Vec<_>>();

                let Some(&first) = predecessors
                    .iter()
                    .find(|predecessor| immediate_dominators.contains_key(predecessor))
                else {
                    continue;
                };

                let mut new_immediate_dominator = first;

                for &predecessor in &predecessors {
                    if predecessor == first || !immediate_dominators.contains_key(&predecessor) {
                        continue;
                    }

                    new_immediate_dominator = intersect(
                        predecessor,
                        new_immediate_dominator,
                        &immediate_dominators,
                        &order,
                    )
                }

                if immediate_dominators.get(&block) != Some(&new_immediate_dominator) {
                    immediate_dominators.insert(block, new_immediate_dominator);
                    changed = true;
                }
            }

            if !changed {
                break;
            }
        }

        let mut children = reverse_postorder
            .iter()
            .copied()
            .map(|block| (block, Vec::new()))
            .collect::<FxHashMap<_, _>>();

        for &block in reverse_postorder.iter().skip(1) {
            let parent = immediate_dominators[&block];

            children
                .get_mut(&parent)
                .expect("immediate dominator must be reachable")
                .push(block);
        }

        Self {
            entry,
            immediate_dominators,
            children,
        }
    }

    /// Returns the region entry block.
    pub const fn entry(&self) -> BlockId {
        self.entry
    }

    /// Returns the immediate dominator of a reachable non-entry block.
    ///
    /// The entry has no immediate dominator.
    pub fn immediate_dominator(&self, block: BlockId) -> Option<BlockId> {
        if block == self.entry {
            return None;
        }

        self.immediate_dominators.get(&block).copied()
    }

    /// Returns the blocks immediately dominated by `block`.
    pub fn children(&self, block: BlockId) -> Option<&[BlockId]> {
        self.children.get(&block).map(Vec::as_slice)
    }

    /// Returns whether `dominator` dominates `block`.
    ///
    /// A reachable block dominates itself. Unreachable blocks are not part of
    /// this dominator tree.
    pub fn dominates(&self, dominator: BlockId, block: BlockId) -> bool {
        if !self.immediate_dominators.contains_key(&dominator)
            || !self.immediate_dominators.contains_key(&block)
        {
            return false;
        }

        let mut current = block;

        loop {
            if current == dominator {
                return true;
            }

            let parent = self.immediate_dominators[&current];

            if parent == current {
                return false;
            }

            current = parent
        }
    }
}

fn intersect(
    mut left: BlockId,
    mut right: BlockId,
    immediate_dominators: &FxHashMap<BlockId, BlockId>,
    order: &FxHashMap<BlockId, usize>,
) -> BlockId {
    while left != right {
        while order[&left] > order[&right] {
            left = immediate_dominators[&left];
        }

        while order[&right] > order[&left] {
            right = immediate_dominators[&right];
        }
    }

    left
}

#[cfg(test)]
mod tests {
    use evrel_ir::{
        BlockTarget, ConstantOp, ConstantValue, IfOp, JumpOp, ModuleBuilder, ModuleIr,
        OperationKind, UnwindTarget,
    };

    use super::RegionDominatorTree;
    use crate::analysis::RegionControlFlowGraph;

    #[test]
    fn computes_dominators_for_linear_control_flow() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (entry, middle, exit) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let entry = builder.current_block();
            let middle = builder.create_block();
            let exit = builder.create_block();

            builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(middle, 0))),
                [],
                UnwindTarget::Propagate,
            );
            builder.switch_to_block(middle);
            builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(exit, 0))),
                [],
                UnwindTarget::Propagate,
            );

            (entry, middle, exit)
        };

        let function = module.function(function_id).unwrap();
        let graph = RegionControlFlowGraph::compute(function, function.body_region()).unwrap();
        let dominance = RegionDominatorTree::compute(&graph);

        assert_eq!(dominance.immediate_dominator(entry), None);
        assert_eq!(dominance.immediate_dominator(middle), Some(entry));
        assert_eq!(dominance.immediate_dominator(exit), Some(middle));
        assert!(dominance.dominates(entry, exit));
        assert!(dominance.dominates(middle, exit));
        assert!(!dominance.dominates(exit, middle));
    }

    #[test]
    fn computes_dominators_for_a_diamond() {
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

        assert_eq!(dominance.immediate_dominator(left), Some(entry));
        assert_eq!(dominance.immediate_dominator(right), Some(entry));
        assert_eq!(dominance.immediate_dominator(join), Some(entry));
        assert!(dominance.dominates(entry, join));
        assert!(!dominance.dominates(left, join));
        assert!(!dominance.dominates(right, join));
    }

    #[test]
    fn computes_dominators_for_a_loop_backedge() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (entry, header, body, exit) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let entry = builder.current_block();
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

            (entry, header, body, exit)
        };

        let function = module.function(function_id).unwrap();
        let graph = RegionControlFlowGraph::compute(function, function.body_region()).unwrap();
        let dominance = RegionDominatorTree::compute(&graph);

        assert_eq!(dominance.immediate_dominator(header), Some(entry));
        assert_eq!(dominance.immediate_dominator(body), Some(header));
        assert_eq!(dominance.immediate_dominator(exit), Some(header));
        assert!(dominance.dominates(header, body));
        assert!(dominance.dominates(header, exit));
        assert!(!dominance.dominates(body, exit));
    }

    #[test]
    fn excludes_unreachable_blocks() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (entry, exit, unreachable) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let entry = builder.current_block();
            let exit = builder.create_block();
            let unreachable = builder.create_block();

            builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(exit, 0))),
                [],
                UnwindTarget::Propagate,
            );

            (entry, exit, unreachable)
        };

        let function = module.function(function_id).unwrap();
        let graph = RegionControlFlowGraph::compute(function, function.body_region()).unwrap();
        let dominance = RegionDominatorTree::compute(&graph);

        assert_eq!(dominance.immediate_dominator(exit), Some(entry));
        assert_eq!(dominance.immediate_dominator(unreachable), None);
        assert!(!dominance.dominates(unreachable, unreachable));
        assert!(!dominance.dominates(entry, unreachable));
    }
}
