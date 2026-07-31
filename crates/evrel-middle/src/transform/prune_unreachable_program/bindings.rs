//! Removal of unobserved module bindings with removable initialization cells.

use evrel_ir::{
    BindingId, BindingKind, FunctionEditor, FunctionId, ModuleEditor, ModuleId, ModuleIr,
    OperationId, OperationKind, ProgramBindingId,
};

use crate::analysis::ProgramReachability;

/// Removes unreachable module bindings referenced only by initialization.
///
/// Initializer operands remain in the IR, so ordinary dead-code elimination
/// removes pure producers while preserving producers with observable effects.
/// Bindings with reads, assignments, destructuring, imports, or other binding
/// semantics are retained.
///
/// Returns the number of removed bindings.
pub(super) fn prune(
    module: ModuleId,
    ir: &mut ModuleIr,
    reachability: &ProgramReachability,
) -> usize {
    let plans = ir
        .bindings()
        .filter_map(|(binding, data)| {
            (data.declaring_function() == ir.entry_function()
                && data.kind() != BindingKind::Import
                && !reachability.is_binding_live(ProgramBindingId::new(module, binding)))
            .then(|| plan_binding_removal(ir, binding))
            .flatten()
        })
        .collect::<Vec<_>>();

    if plans.is_empty() {
        return 0;
    }

    let mut bindings = Vec::with_capacity(plans.len());

    for plan in plans {
        for (function, operations) in plan.initializations {
            FunctionEditor::new(
                ir.function_mut(function)
                    .expect("initialization function must remain live"),
            )
            .remove_operations(operations);
        }

        bindings.push(plan.binding);
    }

    let removed = bindings.len();
    ModuleEditor::new(ir).remove_bindings(bindings);

    removed
}

struct BindingRemoval {
    binding: BindingId,
    initializations: Vec<(FunctionId, Vec<OperationId>)>,
}

fn plan_binding_removal(module: &evrel_ir::ModuleIr, binding: BindingId) -> Option<BindingRemoval> {
    let mut initializations = Vec::new();

    for (function_id, function) in module.functions() {
        let mut function_initializations = Vec::new();

        for (operation_id, operation) in function.operations() {
            let mut references_binding = false;
            operation.kind().visit_referenced_bindings(|referenced| {
                references_binding |= referenced == binding;
            });

            if !references_binding {
                continue;
            }

            match operation.kind() {
                OperationKind::InitializeBinding(initialize) if initialize.binding() == binding => {
                    function_initializations.push(operation_id);
                }
                _ => return None,
            }
        }

        if !function_initializations.is_empty() {
            initializations.push((function_id, function_initializations));
        }
    }

    (!initializations.is_empty()).then_some(BindingRemoval {
        binding,
        initializations,
    })
}
