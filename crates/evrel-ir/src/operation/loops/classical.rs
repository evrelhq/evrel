//! Classical JavaScript `for` loop control flow.

use crate::{BindingId, BlockId};

use super::super::{BlockTarget, OperationEffects, OperationSuccessor};

/// Owns the canonical control-flow phases of one classical JavaScript `for`.
///
/// The operation lives in a dedicated loop-host block. Executing it enters the
/// test target. The test transfers to the body or exit, normal body completion
/// transfers to the update block, and normal update completion re-enters the
/// host. `continue` also targets the update block.
///
/// The initializer precedes the host and is structurally referenced so the
/// complete source loop remains recoverable without inventing CFG edges that
/// do not execute at the host.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForOp {
    initializer_block: BlockId,
    pub(super) test_target: BlockTarget,
    body_block: BlockId,
    update_block: BlockId,
    exit_block: BlockId,
    per_iteration_bindings: Box<[BindingId]>,
    labels: Box<[Box<str>]>,
}

impl ForOp {
    /// Creates a canonical classical `for` loop host.
    pub fn new(
        initializer_block: BlockId,
        test_target: BlockTarget,
        body_block: BlockId,
        update_block: BlockId,
        exit_block: BlockId,
        per_iteration_bindings: Box<[BindingId]>,
        labels: Box<[Box<str>]>,
    ) -> Self {
        let phases = [
            initializer_block,
            test_target.block(),
            body_block,
            update_block,
            exit_block,
        ];

        for (index, phase) in phases.iter().enumerate() {
            assert!(
                !phases[..index].contains(phase),
                "canonical for-loop phases must use distinct blocks"
            );
        }

        Self {
            initializer_block,
            test_target,
            body_block,
            update_block,
            exit_block,
            per_iteration_bindings,
            labels,
        }
    }

    /// Returns the block that evaluates the initializer before the host.
    pub const fn initializer_block(&self) -> BlockId {
        self.initializer_block
    }

    /// Returns the executable target that evaluates the loop test.
    pub const fn test_target(&self) -> BlockTarget {
        self.test_target
    }

    /// Returns the loop body's entry block.
    pub const fn body_block(&self) -> BlockId {
        self.body_block
    }

    /// Returns the update block targeted by normal completion and `continue`.
    pub const fn update_block(&self) -> BlockId {
        self.update_block
    }

    /// Returns the block following the complete loop.
    pub const fn exit_block(&self) -> BlockId {
        self.exit_block
    }

    /// Returns header bindings with fresh instances for each iteration.
    pub fn per_iteration_bindings(&self) -> &[BindingId] {
        &self.per_iteration_bindings
    }

    /// Returns source labels in outermost-to-innermost order.
    pub fn labels(&self) -> &[Box<str>] {
        &self.labels
    }

    /// Returns the innermost source label, when present.
    pub fn label(&self) -> Option<&str> {
        self.labels.last().map(Box::as_ref)
    }

    /// Returns the operation's executable successors.
    pub fn successors(&self) -> Vec<OperationSuccessor> {
        vec![OperationSuccessor::new(self.test_target, 0)]
    }

    /// Returns phase blocks retained structurally rather than as host edges.
    pub fn structural_blocks(&self) -> Vec<BlockId> {
        vec![
            self.initializer_block,
            self.body_block,
            self.update_block,
            self.exit_block,
        ]
    }

    pub(crate) const fn effects(&self) -> OperationEffects {
        OperationEffects::NONE
    }

    pub(crate) const fn operand_count(&self) -> usize {
        self.test_target.argument_count()
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use crate::{BindingId, BlockId, BlockTarget, MemoryEffects, OperationEffects, OperationKind};

    use super::ForOp;

    #[test]
    fn owns_canonical_loop_phases_and_enters_the_test() {
        let initializer = BlockId::from_index(1);
        let test = BlockTarget::new(BlockId::from_index(2), 2);
        let body = BlockId::from_index(3);
        let update = BlockId::from_index(4);
        let exit = BlockId::from_index(5);
        let binding = BindingId::from_index(0);
        let operation = ForOp::new(
            initializer,
            test,
            body,
            update,
            exit,
            Box::new([binding]),
            Box::new(["outer".into()]),
        );
        let successors = operation.successors();

        assert_eq!(operation.initializer_block(), initializer);
        assert_eq!(operation.test_target(), test);
        assert_eq!(operation.body_block(), body);
        assert_eq!(operation.update_block(), update);
        assert_eq!(operation.exit_block(), exit);
        assert_eq!(operation.per_iteration_bindings(), [binding]);
        assert_eq!(operation.label(), Some("outer"));
        assert_eq!(successors.len(), 1);
        assert_eq!(successors[0].target(), test);
        assert_eq!(successors[0].argument_operand_range(), 0..2);
        assert_eq!(
            operation.structural_blocks(),
            [initializer, body, update, exit]
        );
        assert_eq!(operation.operand_count(), 2);
        assert_eq!(operation.result_count(), 0);
        assert_eq!(operation.effects(), OperationEffects::NONE);
        assert_eq!(
            OperationKind::For(operation).intrinsic_memory_effects(),
            MemoryEffects::NONE
        );
    }

    #[test]
    #[should_panic(expected = "canonical for-loop phases must use distinct blocks")]
    fn rejects_aliased_phase_blocks() {
        let shared = BlockId::from_index(1);

        ForOp::new(
            shared,
            BlockTarget::new(shared, 0),
            BlockId::from_index(2),
            BlockId::from_index(3),
            BlockId::from_index(4),
            Box::new([]),
            Box::new([]),
        );
    }
}
