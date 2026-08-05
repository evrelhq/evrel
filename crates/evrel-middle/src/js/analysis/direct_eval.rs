use evrel_js_ir::{JsFunctionIr, JsModuleIr, OperationId, OperationKind, ValueDefinition};

/// Conservatively recognizes a call that may use direct-eval semantics.
pub(super) fn is_direct_eval_call(
    module: &JsModuleIr,
    function: &JsFunctionIr,
    operation: OperationId,
) -> bool {
    let Some(operation) = function.operation(operation) else {
        return false;
    };
    let OperationKind::Call(call) = operation.kind() else {
        return false;
    };
    let Some(callee) = call
        .callee_operand_index()
        .and_then(|index| operation.operands().get(index))
    else {
        return false;
    };
    let Some(ValueDefinition::OperationResult {
        operation: callee_operation,
        ..
    }) = function.value(*callee).map(|value| value.definition())
    else {
        return false;
    };

    function
        .operation(*callee_operation)
        .is_some_and(|operation| match operation.kind() {
            OperationKind::LoadGlobal(global) => global.name() == "eval",
            OperationKind::LoadBinding(load) => module
                .binding(load.binding())
                .is_some_and(|binding| binding.name() == "eval"),
            _ => false,
        })
}
