//! JavaScript code-generation errors.

use evrel_ir::{BindingId, BlockId, FunctionId, OperationId, PrivateNameId, RegionId, ValueId};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum JsCodegenError {
    #[error("unknown function {function:?}")]
    UnknownFunction { function: FunctionId },

    #[error("function {function:?} has an invalid kind for this JavaScript construct")]
    InvalidFunctionKind { function: FunctionId },

    #[error("unknown private name {private_name:?}")]
    UnknownPrivateName { private_name: PrivateNameId },

    #[error("unknown binding {binding:?}")]
    UnknownBinding { binding: BindingId },

    #[error("unknown block {block:?}")]
    UnknownBlock { block: BlockId },

    #[error("unknown region {region:?}")]
    UnknownRegion { region: RegionId },

    #[error("unknown operation {operation:?}")]
    UnknownOperation { operation: OperationId },

    #[error("unknown value {value:?}")]
    UnknownValue { value: ValueId },

    #[error("operation {operation:?} has malformed operands")]
    MalformedOperation { operation: OperationId },

    #[error("function {function:?} contains unsupported control flow ({reason})")]
    UnsupportedControlFlow {
        function: FunctionId,
        reason: &'static str,
    },

    #[error("region {region:?} cannot yet be emitted as an expression")]
    UnsupportedExpressionRegion { region: RegionId },

    #[error(
        "a classical for-loop header cannot be represented without changing semantics ({reason})"
    )]
    UnsupportedForHeader { reason: &'static str },

    #[error("operation {operation:?} is not supported by JavaScript codegen ({reason})")]
    UnsupportedOperation {
        operation: OperationId,
        reason: &'static str,
    },

    #[error("value {value:?} cannot yet be emitted")]
    UnsupportedValue { value: ValueId },

    #[error("function {function:?} has no JavaScript output plan")]
    MissingFunctionPlan { function: FunctionId },
}
