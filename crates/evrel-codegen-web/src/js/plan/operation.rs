use evrel_js_ir::{BindingId, FunctionId, ValueId};

/// Complete JavaScript statement decisions for one IR operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JsOperationPlan {
    statement: JsOperationStatementPlan,
    result_destinations: Box<[ValueId]>,
}

impl JsOperationPlan {
    pub(crate) const fn new(
        statement: JsOperationStatementPlan,
        result_destinations: Box<[ValueId]>,
    ) -> Self {
        Self {
            statement,
            result_destinations,
        }
    }

    pub(crate) const fn statement(&self) -> JsOperationStatementPlan {
        self.statement
    }

    /// Returns the JavaScript destinations that receive successful results.
    pub(crate) const fn result_destinations(&self) -> &[ValueId] {
        &self.result_destinations
    }
}

/// The statement form selected for an IR operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JsOperationStatementPlan {
    /// Use the ordinary statement emitter for the operation.
    Ordinary,

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
