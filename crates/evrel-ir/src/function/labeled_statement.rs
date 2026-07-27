//! Source-structured labeled-statement metadata.

use crate::BlockId;

/// Describes one group of consecutive JavaScript labels.
///
/// Labels are stored from outermost to innermost. Executable control flow
/// remains represented by ordinary block terminators.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LabeledStatementData {
    labels: Box<[Box<str>]>,
    body_block: BlockId,
    completion_block: BlockId,
}

impl LabeledStatementData {
    pub fn new(
        labels: impl Into<Box<[Box<str>]>>,
        body_block: BlockId,
        completion_block: BlockId,
    ) -> Self {
        let labels = labels.into();

        assert!(
            !labels.is_empty(),
            "a labeled statement must have at least one label"
        );
        assert_ne!(
            body_block, completion_block,
            "a labeled statement body and completion must differ"
        );

        Self {
            labels,
            body_block,
            completion_block,
        }
    }

    /// Returns labels in outermost-to-innermost order.
    pub fn labels(&self) -> &[Box<str>] {
        &self.labels
    }

    /// Returns the labeled body's entry block.
    pub const fn body_block(&self) -> BlockId {
        self.body_block
    }

    /// Returns the target of labeled breaks and normal completion.
    pub const fn completion_block(&self) -> BlockId {
        self.completion_block
    }

    pub(crate) const fn referenced_blocks(&self) -> [BlockId; 2] {
        [self.body_block, self.completion_block]
    }
}
