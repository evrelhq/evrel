//! Exceptional control-flow metadata.

use crate::{BlockId, ExceptionHandlerId};

/// Describes how exceptional control enters a handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExceptionHandlerKind {
    Catch,
    Finally,
}

/// Describes one exceptional control-flow destination.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExceptionHandlerData {
    kind: ExceptionHandlerKind,
    parent: Option<ExceptionHandlerId>,
    entry_block: BlockId,
}

impl ExceptionHandlerData {
    /// Creates exception-handler metadata.
    pub const fn new(
        kind: ExceptionHandlerKind,
        parent: Option<ExceptionHandlerId>,
        entry_block: BlockId,
    ) -> Self {
        Self {
            kind,
            parent,
            entry_block,
        }
    }

    /// Returns how exceptional control enters this handler.
    pub const fn kind(&self) -> ExceptionHandlerKind {
        self.kind
    }

    /// Returns the lexically enclosing handler, when present.
    pub const fn parent(&self) -> Option<ExceptionHandlerId> {
        self.parent
    }

    /// Returns the handler's entry block.
    pub const fn entry_block(&self) -> BlockId {
        self.entry_block
    }
}

#[cfg(test)]
mod tests {
    use crate::{BlockId, ExceptionHandlerId};

    use super::{ExceptionHandlerData, ExceptionHandlerKind};

    #[test]
    fn describes_a_nested_exception_handler() {
        let parent = ExceptionHandlerId::from_index(1);
        let entry = BlockId::from_index(3);
        let handler = ExceptionHandlerData::new(ExceptionHandlerKind::Catch, Some(parent), entry);

        assert_eq!(handler.kind(), ExceptionHandlerKind::Catch);
        assert_eq!(handler.parent(), Some(parent));
        assert_eq!(handler.entry_block(), entry);
    }
}
