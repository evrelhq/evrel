//! Basic blocks in a function's control-flow graph.

use crate::{OperationId, RegionId, ValueId};

/// Describes how a block parameter receives its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockParameterSource {
    /// A value created by the predecessor's control-flow operation.
    Produced,

    /// An existing SSA value forwarded by an ordinary control-flow edge.
    Forwarded,

    /// A thrown JavaScript value supplied by exceptional control flow.
    Exception,
}

/// One SSA value received when control enters a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockParameter {
    source: BlockParameterSource,
    value: ValueId,
}

impl BlockParameter {
    pub(crate) const fn new(source: BlockParameterSource, value: ValueId) -> Self {
        Self { source, value }
    }

    /// Returns how the parameter receives its value.
    pub const fn source(self) -> BlockParameterSource {
        self.source
    }

    /// Returns the received SSA value.
    pub const fn value(self) -> ValueId {
        self.value
    }
}

/// Data stored for a basic block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicBlockData {
    region: RegionId,
    parameters: Vec<BlockParameter>,
    operations: Vec<OperationId>,
    terminator: Option<OperationId>,
}

impl BasicBlockData {
    pub(crate) fn new(region: RegionId) -> Self {
        Self {
            region,
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: None,
        }
    }

    /// Returns the region that owns this block.
    pub const fn region(&self) -> RegionId {
        self.region
    }

    /// Returns the values received when control enters this block.
    pub fn parameters(&self) -> &[BlockParameter] {
        &self.parameters
    }

    /// Returns the operations in program order.
    pub fn operations(&self) -> &[OperationId] {
        &self.operations
    }

    /// Returns the operation that transfers control out of this block.
    pub const fn terminator(&self) -> Option<OperationId> {
        self.terminator
    }

    pub(crate) fn add_parameter(&mut self, parameter: BlockParameter) {
        self.parameters.push(parameter);
    }

    pub(crate) fn remove_parameter(&mut self, parameter_index: usize) -> BlockParameter {
        self.parameters.remove(parameter_index)
    }

    pub(crate) fn append_operation(&mut self, operation: OperationId) {
        assert!(
            self.terminator.is_none(),
            "cannot append an operation after a block terminator"
        );

        self.operations.push(operation);
    }

    pub(crate) fn remove_operation(&mut self, operation: OperationId) {
        let index = self
            .operations
            .iter()
            .position(|&candidate| candidate == operation)
            .expect("block does not contain the requested operation");

        self.operations.remove(index);
    }

    pub(crate) fn retain_operations(&mut self, mut keep: impl FnMut(OperationId) -> bool) -> usize {
        let previous_len = self.operations.len();
        self.operations.retain(|operation| keep(*operation));

        previous_len - self.operations.len()
    }

    pub(crate) fn take_operations(&mut self) -> Vec<OperationId> {
        std::mem::take(&mut self.operations)
    }

    pub(crate) fn extend_operations(&mut self, operations: impl IntoIterator<Item = OperationId>) {
        assert!(
            self.terminator.is_none(),
            "cannot append operations after a block terminator",
        );

        self.operations.extend(operations);
    }

    pub(crate) fn set_terminator(&mut self, operation: OperationId) {
        assert!(
            self.terminator.replace(operation).is_none(),
            "a block cannot have more than one terminator"
        );
    }

    pub(crate) fn take_terminator(&mut self) -> Option<OperationId> {
        self.terminator.take()
    }
}
