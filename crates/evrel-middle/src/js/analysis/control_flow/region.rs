//! Ordinary control flow within one IR region.

use evrel_js_ir::{BlockId, JsFunctionIr, OperationId, RegionId};
use rustc_hash::{FxHashMap, FxHashSet};

/// Identifies one executable edge in a regional CFG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ControlFlowEdgeId(usize);

/// One terminator successor in a regional CFG.
///
/// The successor index is retained because different successors may target the
/// same block while carrying different arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ControlFlowEdge {
    source: BlockId,
    target: BlockId,
    terminator: OperationId,
    successor_index: usize,
}

impl ControlFlowEdge {
    /// Returns the block transferring control.
    pub const fn source(self) -> BlockId {
        self.source
    }

    /// Returns the block receiving control.
    pub const fn target(self) -> BlockId {
        self.target
    }

    /// Returns the terminator owning this successor.
    pub const fn terminator(self) -> OperationId {
        self.terminator
    }

    /// Returns this edge's position in the terminator's successor list.
    pub const fn successor_index(self) -> usize {
        self.successor_index
    }
}

/// Ordinary executable control flow within one region.
///
/// This graph deliberately excludes structured ownership. Locally handled
/// exceptions are ordinary explicit terminator successors.
#[derive(Debug, Clone)]
pub struct RegionControlFlowGraph {
    region: RegionId,
    entry: BlockId,
    blocks: Vec<BlockId>,
    edges: Vec<ControlFlowEdge>,
    predecessor_edges: FxHashMap<BlockId, Vec<ControlFlowEdgeId>>,
    successor_edges: FxHashMap<BlockId, Vec<ControlFlowEdgeId>>,
    reachable: FxHashSet<BlockId>,
    reverse_postorder: Vec<BlockId>,
}

impl RegionControlFlowGraph {
    /// Computes ordinary control flow for one region.
    pub fn compute(function: &JsFunctionIr, region: RegionId) -> Self {
        let region_data = function
            .region(region)
            .expect("control-flow analysis requires a live region");

        let entry = region_data.entry_block();
        let blocks = region_data.blocks().to_vec();
        let mut edges = Vec::new();
        let mut predecessor_edges = FxHashMap::default();
        let mut successor_edges = FxHashMap::default();

        for &block in &blocks {
            predecessor_edges.insert(block, Vec::new());
            successor_edges.insert(block, Vec::new());
        }

        for &source in &blocks {
            let block = function
                .block(source)
                .expect("region must reference a live block");

            let Some(terminator) = block.terminator() else {
                continue;
            };

            let operation = function
                .operation(terminator)
                .expect("block terminator must remain live");

            for (successor_index, successor) in operation.successors().into_iter().enumerate() {
                let target = successor.target().block();

                assert_eq!(
                    function.block_region(target),
                    Some(region),
                    "regional CFG edges cannot cross region boundaries",
                );

                let edge = ControlFlowEdgeId(edges.len());

                edges.push(ControlFlowEdge {
                    source,
                    target,
                    terminator,
                    successor_index,
                });

                successor_edges
                    .get_mut(&source)
                    .expect("source block must belong to the region")
                    .push(edge);

                predecessor_edges
                    .get_mut(&target)
                    .expect("target block must belong to the region")
                    .push(edge);
            }
        }

        let (reachable, reverse_postorder) =
            compute_reverse_postorder(entry, &edges, &successor_edges);

        Self {
            region,
            entry,
            blocks,
            edges,
            predecessor_edges,
            successor_edges,
            reachable,
            reverse_postorder,
        }
    }

    /// Returns the represented region.
    pub const fn region(&self) -> RegionId {
        self.region
    }

    /// Returns the region entry block.
    pub const fn entry(&self) -> BlockId {
        self.entry
    }

    /// Returns blocks in deterministic region layout order.
    pub fn blocks(&self) -> &[BlockId] {
        &self.blocks
    }

    /// Returns executable edges in deterministic construction order.
    pub fn edges(&self) -> &[ControlFlowEdge] {
        &self.edges
    }

    /// Returns an edge by ID.
    pub fn edge(&self, edge: ControlFlowEdgeId) -> Option<&ControlFlowEdge> {
        self.edges.get(edge.0)
    }

    /// Returns edges entering a block.
    pub fn predecessor_edges(&self, block: BlockId) -> Option<&[ControlFlowEdgeId]> {
        self.predecessor_edges.get(&block).map(Vec::as_slice)
    }

    /// Returns edges leaving a block.
    pub fn successor_edges(&self, block: BlockId) -> Option<&[ControlFlowEdgeId]> {
        self.successor_edges.get(&block).map(Vec::as_slice)
    }

    /// Returns whether the entry block can reach this block.
    pub fn is_reachable(&self, block: BlockId) -> bool {
        self.reachable.contains(&block)
    }

    /// Returns reachable blocks in reverse postorder.
    pub fn reverse_postorder(&self) -> &[BlockId] {
        &self.reverse_postorder
    }

    /// Iterates over blocks not reachable from the region entry.
    pub fn unreachable_blocks(&self) -> impl Iterator<Item = BlockId> + '_ {
        self.blocks
            .iter()
            .copied()
            .filter(|block| !self.reachable.contains(block))
    }
}

fn compute_reverse_postorder(
    entry: BlockId,
    edges: &[ControlFlowEdge],
    successors: &FxHashMap<BlockId, Vec<ControlFlowEdgeId>>,
) -> (FxHashSet<BlockId>, Vec<BlockId>) {
    let mut reachable = FxHashSet::default();
    let mut postorder = Vec::new();
    let mut stack = vec![(entry, 0usize)];

    reachable.insert(entry);

    while let Some(&(block, next_successor)) = stack.last() {
        let block_successors = successors
            .get(&block)
            .expect("reachable block must belong to the region");

        let Some(&edge) = block_successors.get(next_successor) else {
            stack.pop();
            postorder.push(block);
            continue;
        };

        stack.last_mut().expect("DFS frame must remain present").1 += 1;

        let target = edges[edge.0].target();

        if reachable.insert(target) {
            stack.push((target, 0));
        }
    }

    postorder.reverse();

    (reachable, postorder)
}
