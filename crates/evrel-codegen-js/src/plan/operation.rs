use evrel_ir::{BindingId, FunctionId};

/// A statement-level emission decision for an IR operation.
///
/// Most operations use their ordinary emitter and therefore have no entry in
/// this plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JsOperationPlan {
    /// Emit no statement because every use is represented directly.
    Omitted,

    /// Emit the created function as a declaration bound to `binding`.
    FunctionDeclaration {
        function: FunctionId,
        binding: BindingId,
    },

    /// Declare the operation result with `var` instead of assigning a temporary.
    VarDeclaration,
}
