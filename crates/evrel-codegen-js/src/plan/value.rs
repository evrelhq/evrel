//! JavaScript representations of Evrel IR values.

use evrel_ir::BindingId;

use super::JsLocalId;

/// The JavaScript representation selected for an IR value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JsValueRepresentation {
    /// Recreate a context-free expression at each use.
    ///
    /// Example: an IR numeric constant is emitted directly as `1`.
    Inline,

    /// Emit a function or class expression at its single syntactic use.
    ///
    /// Example: a created function used only as a call target becomes
    /// `(function () {})()`.
    CreationAtUse,

    /// Emit the original global `eval` reference directly at its call site.
    ///
    /// This preserves direct-eval semantics; storing it in a temporary would
    /// instead produce an indirect eval call.
    DirectEval,

    /// Read and write the source-level JavaScript binding.
    Binding(BindingId),

    /// Store the SSA value in a generated JavaScript local.
    Temporary(JsLocalId),
}
