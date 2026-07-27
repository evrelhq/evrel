//! Structured JavaScript `try` control flow.

use crate::BlockId;

use super::{BlockTarget, OperationEffects, OperationSuccessor};

/// Preserves one source-level `try` statement.
///
/// Exceptional transfer is represented separately by function exception-handler
/// metadata. This operation retains the structure needed for JavaScript codegen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TryOp {
    pub(super) try_target: BlockTarget,
    catch_block: Option<BlockId>,
    finally_block: Option<BlockId>,
    completion_block: BlockId,
}

impl TryOp {
    pub fn new(
        try_target: BlockTarget,
        catch_block: Option<BlockId>,
        finally_block: Option<BlockId>,
        completion_block: BlockId,
    ) -> Self {
        assert!(
            catch_block.is_some() || finally_block.is_some(),
            "try must have a catch or finally clause"
        );

        Self {
            try_target,
            catch_block,
            finally_block,
            completion_block,
        }
    }

    pub const fn try_target(&self) -> BlockTarget {
        self.try_target
    }

    pub const fn catch_block(&self) -> Option<BlockId> {
        self.catch_block
    }

    pub const fn finally_block(&self) -> Option<BlockId> {
        self.finally_block
    }

    pub const fn completion_block(&self) -> BlockId {
        self.completion_block
    }

    /// Returns the ordinary executable successor.
    pub fn successors(&self) -> Vec<OperationSuccessor> {
        vec![OperationSuccessor::new(self.try_target, 0)]
    }

    /// Returns non-successor blocks structurally owned by this statement.
    pub fn structural_blocks(&self) -> Vec<BlockId> {
        self.catch_block
            .into_iter()
            .chain(self.finally_block)
            .chain([self.completion_block])
            .collect()
    }

    pub const fn effects(&self) -> OperationEffects {
        OperationEffects::NONE
    }

    pub(crate) const fn operand_count(&self) -> usize {
        self.try_target.argument_count()
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use crate::{BlockId, BlockTarget};

    use super::TryOp;

    #[test]
    fn preserves_try_statement_structure() {
        let try_block = BlockId::from_index(1);
        let catch_block = BlockId::from_index(2);
        let finally_block = BlockId::from_index(3);
        let completion_block = BlockId::from_index(4);
        let operation = TryOp::new(
            BlockTarget::new(try_block, 0),
            Some(catch_block),
            Some(finally_block),
            completion_block,
        );

        assert_eq!(operation.try_target().block(), try_block);
        assert_eq!(operation.catch_block(), Some(catch_block));
        assert_eq!(operation.finally_block(), Some(finally_block));
        assert_eq!(operation.completion_block(), completion_block);
        assert_eq!(
            operation.structural_blocks(),
            [catch_block, finally_block, completion_block]
        );
        assert_eq!(operation.successors().len(), 1);
        assert_eq!(operation.operand_count(), 0);
        assert_eq!(operation.result_count(), 0);
    }

    #[test]
    #[should_panic(expected = "try must have a catch or finally clause")]
    fn rejects_try_without_a_handler() {
        TryOp::new(
            BlockTarget::new(BlockId::from_index(1), 0),
            None,
            None,
            BlockId::from_index(2),
        );
    }
}
