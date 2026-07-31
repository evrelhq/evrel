//! Source-structured JavaScript loop operations.
//!
//! Every source loop is represented by one terminator operation. The
//! operation is the single source of truth for source structure; executable
//! edges remain ordinary operation successors, while non-executable phase
//! references are exposed as structural blocks.

mod classical;
mod conditional;
mod for_in;
mod for_of;

use crate::{BindingId, BlockId};

use super::OperationData;

pub use classical::ForOp;
pub use conditional::{DoWhileOp, WhileOp};
pub use for_in::ForInOp;
pub use for_of::{ForOfKind, ForOfOp};

/// The JavaScript syntax represented by a loop operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoopKind {
    While,
    DoWhile,
    For,
    ForIn,
    ForOf,
}

/// A borrowed, uniform view of one source-structured loop operation.
///
/// This view deliberately does not model compiler-discovered natural loops.
/// Those are derived from CFG backedges and may exist without corresponding
/// JavaScript loop syntax.
#[derive(Debug, Clone, Copy)]
pub enum LoopOperation<'a> {
    While {
        operation_block: BlockId,
        operation: &'a WhileOp,
    },
    DoWhile {
        operation_block: BlockId,
        operation: &'a DoWhileOp,
    },
    For {
        operation_block: BlockId,
        operation: &'a ForOp,
    },
    ForIn {
        operation_block: BlockId,
        operation: &'a ForInOp,
    },
    ForOf {
        operation_block: BlockId,
        operation: &'a ForOfOp,
    },
}

impl<'a> LoopOperation<'a> {
    pub(crate) fn from_operation(operation: &'a OperationData) -> Option<Self> {
        let operation_block = operation.block();

        match operation.kind() {
            super::OperationKind::While(operation) => Some(Self::While {
                operation_block,
                operation,
            }),
            super::OperationKind::DoWhile(operation) => Some(Self::DoWhile {
                operation_block,
                operation,
            }),
            super::OperationKind::For(operation) => Some(Self::For {
                operation_block,
                operation,
            }),
            super::OperationKind::ForIn(operation) => Some(Self::ForIn {
                operation_block,
                operation,
            }),
            super::OperationKind::ForOf(operation) => Some(Self::ForOf {
                operation_block,
                operation,
            }),
            _ => None,
        }
    }

    /// Returns the JavaScript loop form.
    pub const fn kind(self) -> LoopKind {
        match self {
            Self::While { .. } => LoopKind::While,
            Self::DoWhile { .. } => LoopKind::DoWhile,
            Self::For { .. } => LoopKind::For,
            Self::ForIn { .. } => LoopKind::ForIn,
            Self::ForOf { .. } => LoopKind::ForOf,
        }
    }

    /// Returns the block containing the loop operation itself.
    pub const fn operation_block(self) -> BlockId {
        match self {
            Self::While {
                operation_block, ..
            }
            | Self::DoWhile {
                operation_block, ..
            }
            | Self::For {
                operation_block, ..
            }
            | Self::ForIn {
                operation_block, ..
            }
            | Self::ForOf {
                operation_block, ..
            } => operation_block,
        }
    }

    /// Returns the first block belonging to the source loop.
    pub const fn entry_block(self) -> BlockId {
        match self {
            Self::While { operation, .. } => operation.test_block(),
            Self::ForIn {
                operation_block, ..
            }
            | Self::ForOf {
                operation_block, ..
            } => operation_block,
            Self::DoWhile { operation, .. } => operation.body_target().block(),
            Self::For { operation, .. } => operation.initializer_block(),
        }
    }

    /// Returns the first block that evaluates a conditional loop test.
    pub const fn test_block(self) -> Option<BlockId> {
        match self {
            Self::While { operation, .. } => Some(operation.test_block()),
            Self::DoWhile { operation, .. } => Some(operation.test_block()),
            Self::For { operation, .. } => Some(operation.test_target().block()),
            Self::ForIn { .. } | Self::ForOf { .. } => None,
        }
    }

    /// Returns the initializer block of a classical `for` loop.
    pub const fn initializer_block(self) -> Option<BlockId> {
        match self {
            Self::For { operation, .. } => Some(operation.initializer_block()),
            _ => None,
        }
    }

    /// Returns the loop body's entry block.
    pub const fn body_block(self) -> BlockId {
        match self {
            Self::While { operation, .. } => operation.body_target().block(),
            Self::DoWhile { operation, .. } => operation.body_target().block(),
            Self::For { operation, .. } => operation.body_block(),
            Self::ForIn { operation, .. } => operation.body_target().block(),
            Self::ForOf { operation, .. } => operation.body_target().block(),
        }
    }

    /// Returns the block targeted by `continue`.
    pub const fn continue_block(self) -> BlockId {
        match self {
            Self::While { operation, .. } => operation.test_block(),
            Self::DoWhile { operation, .. } => operation.test_block(),
            Self::ForIn {
                operation_block, ..
            }
            | Self::ForOf {
                operation_block, ..
            } => operation_block,
            Self::For { operation, .. } => operation.update_block(),
        }
    }

    /// Returns the block targeted by normal loop exit and `break`.
    pub const fn exit_block(self) -> BlockId {
        match self {
            Self::While { operation, .. } => operation.exit_target().block(),
            Self::DoWhile { operation, .. } => operation.exit_target().block(),
            Self::For { operation, .. } => operation.exit_block(),
            Self::ForIn { operation, .. } => operation.exit_target().block(),
            Self::ForOf { operation, .. } => operation.exit_target().block(),
        }
    }

    /// Returns header bindings with fresh instances for each iteration.
    pub fn per_iteration_bindings(self) -> &'a [BindingId] {
        match self {
            Self::For { operation, .. } => operation.per_iteration_bindings(),
            Self::ForIn { operation, .. } => operation.per_iteration_bindings(),
            Self::ForOf { operation, .. } => operation.per_iteration_bindings(),
            Self::While { .. } | Self::DoWhile { .. } => &[],
        }
    }

    /// Returns source labels in outermost-to-innermost order.
    pub fn labels(self) -> &'a [Box<str>] {
        match self {
            Self::While { operation, .. } => operation.labels(),
            Self::DoWhile { operation, .. } => operation.labels(),
            Self::For { operation, .. } => operation.labels(),
            Self::ForIn { operation, .. } => operation.labels(),
            Self::ForOf { operation, .. } => operation.labels(),
        }
    }

    /// Returns the innermost source label, when present.
    pub fn label(self) -> Option<&'a str> {
        self.labels().last().map(Box::as_ref)
    }

    /// Returns the iterator protocol of a `for...of` loop.
    pub const fn for_of_kind(self) -> Option<ForOfKind> {
        match self {
            Self::ForOf { operation, .. } => Some(operation.kind()),
            _ => None,
        }
    }

    /// Returns every block whose identity is part of this loop's structure.
    pub fn blocks(self) -> impl Iterator<Item = BlockId> {
        self.initializer_block()
            .into_iter()
            .chain(self.test_block())
            .chain([
                self.operation_block(),
                self.body_block(),
                self.continue_block(),
                self.exit_block(),
            ])
    }
}
