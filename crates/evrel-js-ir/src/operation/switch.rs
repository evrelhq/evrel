//! Structured JavaScript `switch` control flow.

use crate::{BlockId, RegionId};

use super::{BlockTarget, OperationEffects, OperationSuccessor};

/// One source-ordered JavaScript switch clause.
///
/// A missing test represents `default`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SwitchCase {
    test_region: Option<RegionId>,
    pub(super) target: BlockTarget,
}

impl SwitchCase {
    /// Creates a source clause with its lazy test and body target.
    pub const fn new(test_region: Option<RegionId>, target: BlockTarget) -> Self {
        Self {
            test_region,
            target,
        }
    }

    /// Returns the lazy case-selector region, or `None` for `default`.
    pub const fn test_region(&self) -> Option<RegionId> {
        self.test_region
    }

    /// Returns the clause body's entry target.
    pub const fn target(&self) -> BlockTarget {
        self.target
    }

    /// Returns whether this is the default clause.
    pub const fn is_default(&self) -> bool {
        self.test_region.is_none()
    }
}

/// Selects and enters one JavaScript switch clause.
///
/// Operand layout:
///
/// 1. discriminant
/// 2. case-target arguments in source order
/// 3. no-match target arguments, when there is no default clause
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SwitchOp {
    pub(super) cases: Box<[SwitchCase]>,
    pub(super) no_match_target: Option<BlockTarget>,
    completion_block: BlockId,
    labels: Box<[Box<str>]>,
}

impl SwitchOp {
    /// Creates a structured switch terminator.
    pub fn new(
        cases: impl Into<Box<[SwitchCase]>>,
        no_match_target: Option<BlockTarget>,
        completion_block: BlockId,
        labels: Box<[Box<str>]>,
    ) -> Self {
        let cases = cases.into();
        let default_count = cases.iter().filter(|case| case.is_default()).count();

        assert!(
            default_count <= 1,
            "switch cannot contain multiple default clauses"
        );
        assert_eq!(
            no_match_target.is_some(),
            default_count == 0,
            "switch requires either a default clause or a no-match target"
        );

        Self {
            cases,
            no_match_target,
            completion_block,
            labels,
        }
    }

    /// Returns source clauses in source order.
    pub fn cases(&self) -> &[SwitchCase] {
        &self.cases
    }

    /// Returns the fallback target used when no default clause exists.
    pub const fn no_match_target(&self) -> Option<BlockTarget> {
        self.no_match_target
    }

    /// Returns the block following the complete switch statement.
    pub const fn completion_block(&self) -> BlockId {
        self.completion_block
    }

    /// Returns source labels in outermost-to-innermost order.
    pub fn labels(&self) -> &[Box<str>] {
        &self.labels
    }

    /// Returns the innermost source label, when present.
    pub fn label(&self) -> Option<&str> {
        self.labels.last().map(Box::as_ref)
    }

    /// Returns lazy case-selector regions in semantic evaluation order.
    pub fn regions(&self) -> Vec<RegionId> {
        self.cases
            .iter()
            .filter_map(SwitchCase::test_region)
            .collect()
    }

    /// Returns executable clause and no-match successors.
    pub fn successors(&self) -> Vec<OperationSuccessor> {
        let mut successors =
            Vec::with_capacity(self.cases.len() + usize::from(self.no_match_target.is_some()));
        let mut operand_index = 1;

        for case in &self.cases {
            let target = case.target();

            successors.push(OperationSuccessor::new(target, operand_index));
            operand_index += target.argument_count();
        }

        if let Some(target) = self.no_match_target {
            successors.push(OperationSuccessor::new(target, operand_index));
        }

        successors
    }

    /// Returns the switch's intrinsic effects, excluding contained regions.
    pub const fn effects(&self) -> OperationEffects {
        OperationEffects::NONE
    }

    pub(crate) fn operand_count(&self) -> usize {
        1 + self
            .cases
            .iter()
            .map(|case| case.target().argument_count())
            .sum::<usize>()
            + self
                .no_match_target
                .map_or(0, |target| target.argument_count())
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use crate::{BlockId, BlockTarget, RegionId};

    use super::{SwitchCase, SwitchOp};

    #[test]
    fn preserves_source_cases_and_lazy_selector_order() {
        let first = RegionId::from_index(1);
        let second = RegionId::from_index(2);
        let completion = BlockId::from_index(4);
        let operation = SwitchOp::new(
            [
                SwitchCase::new(Some(first), BlockTarget::new(BlockId::from_index(1), 0)),
                SwitchCase::new(None, BlockTarget::new(BlockId::from_index(2), 0)),
                SwitchCase::new(Some(second), BlockTarget::new(BlockId::from_index(3), 0)),
            ],
            None,
            completion,
            Box::new(["label".into()]),
        );

        assert_eq!(operation.regions(), [first, second]);
        assert_eq!(operation.successors().len(), 3);
        assert_eq!(operation.completion_block(), completion);
        assert_eq!(operation.label(), Some("label"));
        assert_eq!(operation.operand_count(), 1);
        assert_eq!(operation.result_count(), 0);
    }

    #[test]
    fn uses_an_explicit_no_match_successor_without_default() {
        let completion = BlockId::from_index(2);
        let no_match = BlockTarget::new(completion, 0);
        let operation = SwitchOp::new(
            [SwitchCase::new(
                Some(RegionId::from_index(1)),
                BlockTarget::new(BlockId::from_index(1), 0),
            )],
            Some(no_match),
            completion,
            Box::new([]),
        );

        assert_eq!(operation.no_match_target(), Some(no_match));
        assert_eq!(operation.successors().len(), 2);
    }
}
