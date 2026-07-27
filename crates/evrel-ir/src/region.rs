//! Executable control-flow regions within functions.

use crate::{BlockId, OperationId, RegionId};

/// Structural IR entity that controls when a region executes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegionOwner {
    /// The root body of a function.
    FunctionBody,

    /// An executable operation.
    Operation(OperationId),

    /// A source-level function parameter identified by its declaration index.
    FunctionParameter { parameter_index: u32 },
}

/// A control-flow graph with an explicit structural owner.
///
/// Every function owns one root body region. Inline child regions execute in
/// the enclosing function's lexical and dynamic context; they are not functions
/// and cannot be called independently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionData {
    parent: Option<RegionId>,
    owner: Option<RegionOwner>,
    entry_block: BlockId,
    block_order: Vec<BlockId>,
    result_count: usize,
}

impl RegionData {
    pub(crate) fn function_body(entry_block: BlockId) -> Self {
        Self {
            parent: None,
            owner: Some(RegionOwner::FunctionBody),
            entry_block,
            block_order: vec![entry_block],
            result_count: 0,
        }
    }

    pub(crate) fn inline(parent: RegionId, entry_block: BlockId, result_count: usize) -> Self {
        Self {
            parent: Some(parent),
            owner: None,
            entry_block,
            block_order: vec![entry_block],
            result_count,
        }
    }

    /// Returns the lexically enclosing region, if this region is nested.
    pub const fn parent(&self) -> Option<RegionId> {
        self.parent
    }

    /// Returns the structural IR entity that owns this region.
    pub const fn owner(&self) -> Option<RegionOwner> {
        self.owner
    }

    /// Returns the region's entry block.
    pub const fn entry_block(&self) -> BlockId {
        self.entry_block
    }

    /// Returns blocks in deterministic region layout order.
    pub fn blocks(&self) -> &[BlockId] {
        &self.block_order
    }

    /// Returns the number of values supplied on normal completion.
    pub const fn result_count(&self) -> usize {
        self.result_count
    }

    pub(crate) fn append_block(&mut self, block: BlockId) {
        self.block_order.push(block);
    }

    pub(crate) fn attach(&mut self, owner: RegionOwner) {
        assert!(
            self.owner.replace(owner).is_none(),
            "a region cannot have more than one owner"
        );
    }
}
