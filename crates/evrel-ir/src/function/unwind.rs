//! Exceptional control-flow destinations for function operations.

use crate::ExceptionHandlerId;

/// The destination used when an operation raises an exception.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum UnwindTarget {
    /// Propagate the exception through the current function boundary.
    #[default]
    Propagate,

    /// Transfer to a lexically enclosing handler in the current function.
    Handler(ExceptionHandlerId),
}

impl UnwindTarget {
    /// Returns the local handler, or `None` when the exception propagates.
    pub const fn handler(self) -> Option<ExceptionHandlerId> {
        match self {
            Self::Propagate => None,
            Self::Handler(handler) => Some(handler),
        }
    }
}
