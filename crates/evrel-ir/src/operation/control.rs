//! JavaScript control-flow operations.

use std::ops::Range;

use crate::{BlockId, ValueId};

/// One control-flow destination and the values forwarded to its block
/// parameters.
///
/// Forwarded values remain in `OperationData::operands`; this records how many
/// operands belong to this target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockTarget {
    block: BlockId,
    argument_count: usize,
}

impl BlockTarget {
    /// Creates a block target.
    pub const fn new(block: BlockId, argument_count: usize) -> Self {
        Self {
            block,
            argument_count,
        }
    }

    /// Returns the destination block.
    pub const fn block(&self) -> BlockId {
        self.block
    }

    /// Returns the number of values forwarded to the destination.
    pub const fn argument_count(&self) -> usize {
        self.argument_count
    }
}

/// One executable control-flow successor.
///
/// Successor arguments remain in `OperationData::operands`. This records where
/// that target's arguments begin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OperationSuccessor {
    target: BlockTarget,
    first_argument_operand: usize,
    produced_argument_count: usize,
}

impl OperationSuccessor {
    /// Creates an executable successor over one operation-operand range.
    pub const fn new(target: BlockTarget, first_argument_operand: usize) -> Self {
        Self {
            target,
            first_argument_operand,
            produced_argument_count: 0,
        }
    }

    /// Records values created by the control-flow operation for this edge.
    pub const fn with_produced_arguments(mut self, count: usize) -> Self {
        self.produced_argument_count = count;
        self
    }

    /// Returns the destination and its expected argument count.
    pub const fn target(self) -> BlockTarget {
        self.target
    }

    /// Returns the number of block arguments created by this edge.
    pub const fn produced_argument_count(self) -> usize {
        self.produced_argument_count
    }

    /// Returns the operation-operand range forwarded through this successor.
    pub fn argument_operand_range(self) -> Range<usize> {
        let start = self.first_argument_operand;

        start..start + self.target.argument_count()
    }

    /// Returns the operation operands forwarded to this successor.
    pub fn arguments(self, operands: &[ValueId]) -> &[ValueId] {
        operands
            .get(self.argument_operand_range())
            .expect("successor argument range must be valid")
    }
}

/// Transfers control unconditionally to another block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JumpOp {
    pub(super) target: BlockTarget,
}

impl JumpOp {
    /// Creates an unconditional jump.
    pub const fn new(target: BlockTarget) -> Self {
        Self { target }
    }

    /// Returns the jump destination.
    pub const fn target(&self) -> BlockTarget {
        self.target
    }

    pub(crate) const fn operand_count(&self) -> usize {
        self.target.argument_count()
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }
}

/// Selects one of two blocks using JavaScript truthiness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IfOp {
    pub(super) then_target: BlockTarget,
    pub(super) else_target: BlockTarget,
    completion_block: BlockId,
}

impl IfOp {
    /// Creates a structured conditional terminator.
    pub const fn new(
        then_target: BlockTarget,
        else_target: BlockTarget,
        completion_block: BlockId,
    ) -> Self {
        Self {
            then_target,
            else_target,
            completion_block,
        }
    }

    /// Returns the target selected by a truthy condition.
    pub const fn then_target(&self) -> BlockTarget {
        self.then_target
    }

    /// Returns the target selected by a falsy condition.
    pub const fn else_target(&self) -> BlockTarget {
        self.else_target
    }

    /// Returns the block following the complete structured `if`.
    pub const fn completion_block(&self) -> BlockId {
        self.completion_block
    }

    pub(crate) const fn operand_count(&self) -> usize {
        1 + self.then_target.argument_count() + self.else_target.argument_count()
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }
}

/// Returns one value from the current function.
///
/// A source-level `return;` first produces the compiler-defined `undefined`
/// value, so this operation always has exactly one operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReturnOp;

impl ReturnOp {
    /// Creates a return operation.
    pub const fn new() -> Self {
        Self
    }

    pub(crate) const fn operand_count(&self) -> usize {
        1
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }
}

impl Default for ReturnOp {
    fn default() -> Self {
        Self::new()
    }
}

/// Completes an inline region normally and supplies values to its owner.
///
/// This is an IR region boundary and is unrelated to JavaScript generator
/// `yield` expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegionYieldOp {
    value_count: usize,
}

impl RegionYieldOp {
    /// Creates a region completion with the given result arity.
    pub const fn new(value_count: usize) -> Self {
        Self { value_count }
    }

    /// Returns the number of values supplied to the owning operation.
    pub const fn value_count(&self) -> usize {
        self.value_count
    }

    pub(crate) const fn operand_count(&self) -> usize {
        self.value_count
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }
}

/// Throws a JavaScript value.
///
/// This operation has one operand and terminates the current block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThrowOp;

impl ThrowOp {
    /// Creates a throw operation.
    pub const fn new() -> Self {
        Self
    }

    pub(crate) const fn operand_count(&self) -> usize {
        1
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }
}

impl Default for ThrowOp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::{BlockId, ValueId};

    use super::{BlockTarget, IfOp, JumpOp, OperationSuccessor, RegionYieldOp, ReturnOp, ThrowOp};

    #[test]
    fn locates_successor_arguments_in_operation_operands() {
        let target = BlockTarget::new(BlockId::from_index(2), 2);
        let successor = OperationSuccessor::new(target, 1);
        let operands = [
            ValueId::from_index(0),
            ValueId::from_index(1),
            ValueId::from_index(2),
        ];

        assert_eq!(successor.target(), target);
        assert_eq!(successor.produced_argument_count(), 0);
        assert_eq!(successor.arguments(&operands), &operands[1..]);

        let successor = successor.with_produced_arguments(1);
        assert_eq!(successor.produced_argument_count(), 1);
    }

    #[test]
    fn defines_an_unconditional_jump() {
        let block = BlockId::from_index(2);
        let target = BlockTarget::new(block, 1);
        let operation = JumpOp::new(target);

        assert_eq!(operation.target(), target);
        assert_eq!(operation.operand_count(), 1);
        assert_eq!(operation.result_count(), 0);
    }

    #[test]
    fn defines_a_structured_conditional() {
        let then_target = BlockTarget::new(BlockId::from_index(1), 2);
        let else_target = BlockTarget::new(BlockId::from_index(2), 1);
        let completion = BlockId::from_index(3);
        let operation = IfOp::new(then_target, else_target, completion);

        assert_eq!(operation.then_target(), then_target);
        assert_eq!(operation.else_target(), else_target);
        assert_eq!(operation.completion_block(), completion);
        assert_eq!(operation.operand_count(), 4);
        assert_eq!(operation.result_count(), 0);
    }

    #[test]
    fn defines_the_return_shape() {
        let operation = ReturnOp::new();

        assert_eq!(operation.operand_count(), 1);
        assert_eq!(operation.result_count(), 0);
    }

    #[test]
    fn defines_the_region_yield_shape() {
        let operation = RegionYieldOp::new(1);

        assert_eq!(operation.value_count(), 1);
        assert_eq!(operation.operand_count(), 1);
        assert_eq!(operation.result_count(), 0);
    }

    #[test]
    fn defines_the_throw_shape() {
        let operation = ThrowOp::new();

        assert_eq!(operation.operand_count(), 1);
        assert_eq!(operation.result_count(), 0);
    }
}
