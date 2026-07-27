//! JavaScript `for...of` iteration.

use crate::BindingId;

use super::super::{BlockTarget, OperationEffects, OperationSuccessor};

/// Selects synchronous or asynchronous iterator semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForOfKind {
    Synchronous,
    Asynchronous,
}

impl ForOfKind {
    /// Returns whether iteration may suspend while awaiting iterator results.
    pub const fn is_async(self) -> bool {
        matches!(self, Self::Asynchronous)
    }
}

/// Iterates over values produced by JavaScript's iterator protocol.
///
/// The iterable is operand zero. Each execution either transfers to the body
/// target and produces one iteration-value parameter, or transfers to the exit
/// target when iteration is complete. Re-entering this operation advances the
/// same source-level iterator.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForOfOp {
    kind: ForOfKind,
    body_target: BlockTarget,
    exit_target: BlockTarget,
    per_iteration_bindings: Box<[BindingId]>,
    labels: Box<[Box<str>]>,
}

impl ForOfOp {
    /// Creates a `for...of` or `for await...of` loop header.
    pub fn new(
        kind: ForOfKind,
        body_target: BlockTarget,
        exit_target: BlockTarget,
        per_iteration_bindings: Box<[BindingId]>,
        labels: Box<[Box<str>]>,
    ) -> Self {
        Self {
            kind,
            body_target,
            exit_target,
            per_iteration_bindings,
            labels,
        }
    }

    /// Returns the iterator protocol used by the loop.
    pub const fn kind(&self) -> ForOfKind {
        self.kind
    }

    /// Returns the target receiving the next iteration value.
    pub const fn body_target(&self) -> BlockTarget {
        self.body_target
    }

    /// Returns the target used when iteration is complete.
    pub const fn exit_target(&self) -> BlockTarget {
        self.exit_target
    }

    /// Returns header bindings with fresh instances for each iteration.
    pub fn per_iteration_bindings(&self) -> &[BindingId] {
        &self.per_iteration_bindings
    }

    /// Returns source labels in outermost-to-innermost order.
    pub fn labels(&self) -> &[Box<str>] {
        &self.labels
    }

    /// Returns the operation's executable successors.
    pub fn successors(&self) -> Vec<OperationSuccessor> {
        vec![
            OperationSuccessor::new(self.body_target, 1).with_produced_arguments(1),
            OperationSuccessor::new(self.exit_target, 1 + self.body_target.argument_count()),
        ]
    }

    pub(crate) const fn effects(&self) -> OperationEffects {
        match self.kind {
            ForOfKind::Synchronous => OperationEffects::MAY_THROW_AND_OBSERVABLE,
            ForOfKind::Asynchronous => OperationEffects::MAY_THROW_OR_SUSPEND_AND_OBSERVABLE,
        }
    }

    pub(crate) const fn operand_count(&self) -> usize {
        1 + self.body_target.argument_count() + self.exit_target.argument_count()
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use crate::{BlockId, BlockTarget};

    use super::{ForOfKind, ForOfOp};

    #[test]
    fn produces_one_iteration_value_for_the_body_edge() {
        let body = BlockTarget::new(BlockId::from_index(1), 0);
        let exit = BlockTarget::new(BlockId::from_index(2), 0);
        let operation = ForOfOp::new(
            ForOfKind::Synchronous,
            body,
            exit,
            Box::new([]),
            Box::new([]),
        );
        let successors = operation.successors();

        assert_eq!(operation.kind(), ForOfKind::Synchronous);
        assert_eq!(operation.body_target(), body);
        assert_eq!(operation.exit_target(), exit);
        assert_eq!(successors[0].target(), body);
        assert_eq!(successors[0].produced_argument_count(), 1);
        assert_eq!(successors[1].target(), exit);
        assert_eq!(successors[1].produced_argument_count(), 0);
        assert_eq!(operation.operand_count(), 1);
        assert_eq!(operation.result_count(), 0);
    }

    #[test]
    fn classifies_asynchronous_iteration_effects() {
        let operation = ForOfOp::new(
            ForOfKind::Asynchronous,
            BlockTarget::new(BlockId::from_index(1), 0),
            BlockTarget::new(BlockId::from_index(2), 0),
            Box::new([]),
            Box::new([]),
        );
        let effects = operation.effects();

        assert!(operation.kind().is_async());
        assert!(effects.may_throw());
        assert!(effects.may_suspend());
        assert!(effects.may_have_observable_effects());
    }
}
