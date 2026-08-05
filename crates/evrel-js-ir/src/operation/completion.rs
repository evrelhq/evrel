//! Explicit JavaScript completion flow through `finally` clauses.

use crate::BlockId;

use super::{BlockTarget, OperationSuccessor};

/// A pending JavaScript completion carried through a `finally` clause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompletionKind {
    Normal,
    Return,
    Throw,
    Break(BlockId),
    Continue(BlockId),
}

impl CompletionKind {
    /// Returns the eventual control-flow target of a break or continue.
    pub const fn control_target(self) -> Option<BlockId> {
        match self {
            Self::Break(target) | Self::Continue(target) => Some(target),
            Self::Normal | Self::Return | Self::Throw => None,
        }
    }

    pub(crate) const fn payload_count(self) -> usize {
        match self {
            Self::Return | Self::Throw => 1,
            Self::Normal | Self::Break(_) | Self::Continue(_) => 0,
        }
    }
}

/// Enters a `finally` clause while preserving the pending completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnterFinallyOp {
    kind: CompletionKind,
    pub(super) target: BlockTarget,
}

impl EnterFinallyOp {
    pub const fn new(kind: CompletionKind, target: BlockTarget) -> Self {
        Self { kind, target }
    }

    pub const fn kind(&self) -> CompletionKind {
        self.kind
    }

    pub const fn target(&self) -> BlockTarget {
        self.target
    }

    pub(crate) fn successors(&self) -> Vec<OperationSuccessor> {
        vec![
            OperationSuccessor::new(self.target, self.kind.payload_count())
                .with_produced_arguments(1),
        ]
    }

    pub(crate) const fn operand_count(&self) -> usize {
        self.kind.payload_count() + self.target.argument_count()
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }
}

/// One possible completion resumed after a `finally` clause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompletionCase {
    kind: CompletionKind,
    pub(super) target: BlockTarget,
}

impl CompletionCase {
    pub const fn new(kind: CompletionKind, target: BlockTarget) -> Self {
        Self { kind, target }
    }

    pub const fn kind(&self) -> CompletionKind {
        self.kind
    }

    pub const fn target(&self) -> BlockTarget {
        self.target
    }
}

/// Resumes the completion preserved by an enclosing `finally` clause.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResumeCompletionOp {
    pub(super) cases: Box<[CompletionCase]>,
}

impl ResumeCompletionOp {
    pub fn new(cases: impl Into<Box<[CompletionCase]>>) -> Self {
        let cases = cases.into();

        assert!(
            !cases.is_empty(),
            "completion resume requires at least one case"
        );

        for (index, case) in cases.iter().enumerate() {
            assert!(
                !cases[..index]
                    .iter()
                    .any(|candidate| candidate.kind == case.kind),
                "completion resume cases must be unique",
            );
        }

        Self { cases }
    }

    pub fn cases(&self) -> &[CompletionCase] {
        &self.cases
    }

    pub(crate) fn successors(&self) -> Vec<OperationSuccessor> {
        let mut first_argument_operand = 1;

        self.cases
            .iter()
            .map(|case| {
                let successor = OperationSuccessor::new(case.target, first_argument_operand)
                    .with_produced_arguments(case.kind.payload_count());

                first_argument_operand += case.target.argument_count();

                successor
            })
            .collect()
    }

    pub(crate) fn operand_count(&self) -> usize {
        1 + self
            .cases
            .iter()
            .map(|case| case.target.argument_count())
            .sum::<usize>()
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use crate::{BlockId, BlockTarget};

    use super::{CompletionCase, CompletionKind, EnterFinallyOp, ResumeCompletionOp};

    #[test]
    fn enters_finally_with_one_produced_completion() {
        let operation = EnterFinallyOp::new(
            CompletionKind::Return,
            BlockTarget::new(BlockId::from_index(1), 0),
        );
        let successors = operation.successors();

        assert_eq!(operation.operand_count(), 1);
        assert_eq!(successors.len(), 1);
        assert_eq!(successors[0].produced_argument_count(), 1);
    }

    #[test]
    fn resumes_payload_and_payload_free_completions() {
        let operation = ResumeCompletionOp::new([
            CompletionCase::new(
                CompletionKind::Normal,
                BlockTarget::new(BlockId::from_index(1), 0),
            ),
            CompletionCase::new(
                CompletionKind::Return,
                BlockTarget::new(BlockId::from_index(2), 0),
            ),
        ]);
        let successors = operation.successors();

        assert_eq!(operation.operand_count(), 1);
        assert_eq!(successors[0].produced_argument_count(), 0);
        assert_eq!(successors[1].produced_argument_count(), 1);
    }
}
