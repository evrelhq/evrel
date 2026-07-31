//! Elimination of dead module bindings with removable initialization cells.

use evrel_js_ir::{
    BindingId, BindingKind, FunctionEditor, FunctionId, JsModuleIr, ModuleEditor, OperationId,
    OperationKind,
};
use rustc_hash::FxHashSet;

/// Removes dead module bindings referenced only by initialization.
///
/// Initializer operands remain in the IR, so ordinary dead-code elimination
/// removes pure producers while preserving producers with observable effects.
/// Exported bindings and bindings in modules containing possible direct eval
/// are retained, as are bindings with reads, assignments, destructuring,
/// imports, or other binding semantics.
///
/// Returns the number of removed bindings.
pub fn eliminate_dead_bindings(module: &mut JsModuleIr) -> usize {
    if has_possible_direct_eval(module) {
        return 0;
    }

    let exported = module
        .exports()
        .iter()
        .filter_map(|export| export.binding())
        .collect::<FxHashSet<_>>();
    let plans = module
        .bindings()
        .filter_map(|(binding, data)| {
            (data.declaring_function() == module.entry_function()
                && data.kind() != BindingKind::Import
                && !exported.contains(&binding))
            .then(|| plan_binding_removal(module, binding))
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
                module
                    .function_mut(function)
                    .expect("initialization function must remain live"),
            )
            .remove_operations(operations);
        }

        bindings.push(plan.binding);
    }

    let removed = bindings.len();
    ModuleEditor::new(module).remove_bindings(bindings);

    removed
}

fn has_possible_direct_eval(module: &JsModuleIr) -> bool {
    module.functions().any(|(_, function)| {
        function.operations().any(|(_, operation)| {
            matches!(
                operation.kind(),
                OperationKind::LoadGlobal(global) if global.name() == "eval"
            )
        })
    })
}

struct BindingRemoval {
    binding: BindingId,
    initializations: Vec<(FunctionId, Vec<OperationId>)>,
}

fn plan_binding_removal(
    module: &evrel_js_ir::JsModuleIr,
    binding: BindingId,
) -> Option<BindingRemoval> {
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

#[cfg(test)]
mod tests {
    use evrel_js_ir::{
        BindingKind, ConstantOp, ConstantValue, InitializeBindingOp, JsModuleIr, LoadGlobalOp,
        LocationId, ModuleBuilder, ModuleExport, ModuleExportName, OperationId, OperationKind,
        UnwindTarget,
    };

    use super::eliminate_dead_bindings;

    #[test]
    fn removes_an_unexported_binding_but_leaves_its_initializer_value() {
        let mut module = JsModuleIr::new();
        let (binding, value, initialization) = add_initialized_binding(&mut module);
        let entry = module.entry_function();

        assert_eq!(eliminate_dead_bindings(&mut module), 1);
        assert!(module.binding(binding).is_none());

        let function = module
            .function(entry)
            .expect("entry function must remain live");
        assert!(function.operation(value).is_some());
        assert!(function.operation(initialization).is_none());
    }

    #[test]
    fn preserves_a_binding_exposed_by_the_module_interface() {
        let mut module = JsModuleIr::new();
        let (binding, _, initialization) = add_initialized_binding(&mut module);
        ModuleBuilder::new(&mut module).add_export(ModuleExport::local(
            LocationId::UNKNOWN,
            ModuleExportName::Identifier("value".into()),
            binding,
        ));

        assert_eq!(eliminate_dead_bindings(&mut module), 0);
        assert!(module.binding(binding).is_some());
        assert!(
            module
                .function(module.entry_function())
                .expect("entry function must remain live")
                .operation(initialization)
                .is_some()
        );
    }

    #[test]
    fn preserves_module_bindings_visible_to_possible_direct_eval() {
        let mut module = JsModuleIr::new();
        let (binding, _, initialization) = add_initialized_binding(&mut module);
        let entry = module.entry_function();
        ModuleBuilder::new(&mut module)
            .function_builder(entry)
            .append_operation(
                LocationId::UNKNOWN,
                OperationKind::LoadGlobal(LoadGlobalOp::new("eval")),
                [],
                UnwindTarget::Propagate,
            );

        assert_eq!(eliminate_dead_bindings(&mut module), 0);
        assert!(module.binding(binding).is_some());
        assert!(
            module
                .function(entry)
                .expect("entry function must remain live")
                .operation(initialization)
                .is_some()
        );
    }

    fn add_initialized_binding(
        module: &mut JsModuleIr,
    ) -> (evrel_js_ir::BindingId, OperationId, OperationId) {
        let entry = module.entry_function();
        let mut module_builder = ModuleBuilder::new(module);
        let binding = module_builder.create_binding(entry, "value", BindingKind::Const);
        let mut builder = module_builder.function_builder(entry);
        let value = builder.append_operation(
            LocationId::UNKNOWN,
            OperationKind::Constant(ConstantOp::new(ConstantValue::Number(1.0))),
            [],
            UnwindTarget::Propagate,
        );
        let value_result = builder.operation_results(value)[0];
        let initialization = builder.append_operation(
            LocationId::UNKNOWN,
            OperationKind::InitializeBinding(InitializeBindingOp::new(binding)),
            [value_result],
            UnwindTarget::Propagate,
        );

        (binding, value, initialization)
    }
}
