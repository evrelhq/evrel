//! JavaScript function boundary parameters.

use crate::{BindingPattern, ValueId};

/// Describes how a function receives one source-level parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunctionParameterKind {
    /// A positional argument.
    Argument,

    /// The final rest argument.
    Rest,
}

/// One source-level parameter at a JavaScript function boundary.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionParameter {
    kind: FunctionParameterKind,
    target: BindingPattern,
    value: ValueId,
}

impl FunctionParameter {
    pub(crate) const fn new(
        kind: FunctionParameterKind,
        target: BindingPattern,
        value: ValueId,
    ) -> Self {
        Self {
            kind,
            target,
            value,
        }
    }

    /// Returns whether this is an ordinary or rest parameter.
    pub const fn kind(&self) -> FunctionParameterKind {
        self.kind
    }

    /// Returns the binding pattern initialized by the argument.
    pub const fn target(&self) -> &BindingPattern {
        &self.target
    }

    /// Returns the SSA value received at the function boundary.
    pub const fn value(&self) -> ValueId {
        self.value
    }
}
