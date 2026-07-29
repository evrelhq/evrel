//! Conditional JavaScript loop operations.

use crate::BlockId;

use super::super::{BlockTarget, OperationSuccessor};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ConditionalLoopTargets {
    test_block: BlockId,
    body_target: BlockTarget,
    exit_target: BlockTarget,
    labels: Box<[Box<str>]>,
}

impl ConditionalLoopTargets {
    fn new(
        test_block: BlockId,
        body_target: BlockTarget,
        exit_target: BlockTarget,
        labels: Box<[Box<str>]>,
    ) -> Self {
        let phases = [test_block, body_target.block(), exit_target.block()];
        for (index, phase) in phases.iter().enumerate() {
            assert!(
                !phases[..index].contains(phase),
                "conditional loop test, body, and exit must use distinct blocks"
            );
        }

        Self {
            test_block,
            body_target,
            exit_target,
            labels,
        }
    }

    const fn test_block(&self) -> BlockId {
        self.test_block
    }

    const fn body_target(&self) -> BlockTarget {
        self.body_target
    }

    const fn exit_target(&self) -> BlockTarget {
        self.exit_target
    }

    fn labels(&self) -> &[Box<str>] {
        &self.labels
    }

    fn successors(&self) -> Vec<OperationSuccessor> {
        vec![
            OperationSuccessor::new(self.body_target, 1),
            OperationSuccessor::new(self.exit_target, 1 + self.body_target.argument_count()),
        ]
    }

    const fn operand_count(&self) -> usize {
        1 + self.body_target.argument_count() + self.exit_target.argument_count()
    }
}

/// Tests before each iteration and enters the body while the condition is truthy.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WhileOp {
    targets: ConditionalLoopTargets,
}

impl WhileOp {
    /// Creates a `while` terminator.
    pub fn new(
        test_block: BlockId,
        body_target: BlockTarget,
        exit_target: BlockTarget,
        labels: Box<[Box<str>]>,
    ) -> Self {
        Self {
            targets: ConditionalLoopTargets::new(test_block, body_target, exit_target, labels),
        }
    }

    /// Returns the first block that evaluates the loop test.
    pub const fn test_block(&self) -> BlockId {
        self.targets.test_block()
    }

    /// Returns the target selected by a truthy condition.
    pub const fn body_target(&self) -> BlockTarget {
        self.targets.body_target()
    }

    /// Returns the target selected by a falsy condition.
    pub const fn exit_target(&self) -> BlockTarget {
        self.targets.exit_target()
    }

    /// Returns source labels in outermost-to-innermost order.
    pub fn labels(&self) -> &[Box<str>] {
        self.targets.labels()
    }

    /// Returns the operation's executable successors.
    pub fn successors(&self) -> Vec<OperationSuccessor> {
        self.targets.successors()
    }

    pub(crate) fn successor_target_mut(&mut self, successor_index: usize) -> &mut BlockTarget {
        match successor_index {
            0 => &mut self.targets.body_target,
            1 => &mut self.targets.exit_target,
            _ => panic!("while has no successor {successor_index}"),
        }
    }

    pub(crate) const fn operand_count(&self) -> usize {
        self.targets.operand_count()
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }

    /// Returns phase blocks retained structurally rather than as terminator edges.
    pub fn structural_blocks(&self) -> Vec<BlockId> {
        vec![self.test_block()]
    }
}

/// Tests after each iteration and repeats the body while the condition is truthy.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DoWhileOp {
    targets: ConditionalLoopTargets,
}

impl DoWhileOp {
    /// Creates a `do...while` terminator.
    pub fn new(
        test_block: BlockId,
        body_target: BlockTarget,
        exit_target: BlockTarget,
        labels: Box<[Box<str>]>,
    ) -> Self {
        Self {
            targets: ConditionalLoopTargets::new(test_block, body_target, exit_target, labels),
        }
    }

    /// Returns the first block that evaluates the loop test.
    pub const fn test_block(&self) -> BlockId {
        self.targets.test_block()
    }

    /// Returns the target selected by a truthy condition.
    pub const fn body_target(&self) -> BlockTarget {
        self.targets.body_target()
    }

    /// Returns the target selected by a falsy condition.
    pub const fn exit_target(&self) -> BlockTarget {
        self.targets.exit_target()
    }

    /// Returns source labels in outermost-to-innermost order.
    pub fn labels(&self) -> &[Box<str>] {
        self.targets.labels()
    }

    /// Returns the operation's executable successors.
    pub fn successors(&self) -> Vec<OperationSuccessor> {
        self.targets.successors()
    }

    pub(crate) fn successor_target_mut(&mut self, successor_index: usize) -> &mut BlockTarget {
        match successor_index {
            0 => &mut self.targets.body_target,
            1 => &mut self.targets.exit_target,
            _ => panic!("do-while has no successor {successor_index}"),
        }
    }

    pub(crate) const fn operand_count(&self) -> usize {
        self.targets.operand_count()
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }

    /// Returns phase blocks retained structurally rather than as terminator edges.
    pub fn structural_blocks(&self) -> Vec<BlockId> {
        vec![self.test_block()]
    }
}

#[cfg(test)]
mod tests {
    use crate::{BlockId, BlockTarget, OperationKind};

    use super::{DoWhileOp, WhileOp};

    #[test]
    fn while_owns_its_test_entry_successors_and_labels() {
        let test = BlockId::from_index(1);
        let body = BlockTarget::new(BlockId::from_index(2), 1);
        let exit = BlockTarget::new(BlockId::from_index(3), 2);
        let operation = WhileOp::new(test, body, exit, Box::new(["outer".into()]));
        let successors = operation.successors();

        assert_eq!(operation.test_block(), test);
        assert_eq!(
            operation
                .labels()
                .iter()
                .map(Box::as_ref)
                .collect::<Vec<_>>(),
            ["outer"]
        );
        assert_eq!(successors[0].target(), body);
        assert_eq!(successors[0].argument_operand_range(), 1..2);
        assert_eq!(successors[1].target(), exit);
        assert_eq!(successors[1].argument_operand_range(), 2..4);
        assert_eq!(
            OperationKind::While(operation).intrinsic_effects(),
            crate::OperationEffects::NONE
        );
    }

    #[test]
    fn do_while_can_finish_a_multi_block_test() {
        let test_entry = BlockId::from_index(1);
        let operation = DoWhileOp::new(
            test_entry,
            BlockTarget::new(BlockId::from_index(2), 0),
            BlockTarget::new(BlockId::from_index(3), 0),
            Box::new([]),
        );

        assert_eq!(operation.test_block(), test_entry);
        assert_eq!(operation.structural_blocks(), [test_entry]);
    }
}
