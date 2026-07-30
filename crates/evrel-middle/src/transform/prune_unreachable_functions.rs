//! Removal of module-owned functions that can no longer be instantiated.

use evrel_ir::{ModuleEditor, ModuleIr};

use crate::analysis::ModuleFunctionReachability;

/// Removes unreachable functions and the bindings declared by them.
///
/// Returns the number of removed functions.
pub fn prune_unreachable_functions(module: &mut ModuleIr) -> usize {
    let unreachable = {
        let reachability = ModuleFunctionReachability::compute(module);

        module
            .functions()
            .map(|(function, _)| function)
            .filter(|function| !reachability.is_reachable(*function))
            .collect::<Vec<_>>()
    };
    let removed = unreachable.len();

    if !unreachable.is_empty() {
        ModuleEditor::new(module).remove_functions(unreachable);
    }

    removed
}

#[cfg(test)]
mod tests {
    use evrel_ir::{
        BindingKind, CreateFunctionOp, FunctionKind, FunctionMode, ModuleBuilder, ModuleIr,
        OperationKind, UnwindTarget,
    };

    use super::prune_unreachable_functions;

    #[test]
    fn removes_unreachable_functions_and_their_bindings() {
        let mut module = ModuleIr::new();
        let entry = module.entry_function();

        let (reachable, unreachable, unreachable_binding) = {
            let mut builder = ModuleBuilder::new(&mut module);
            let reachable =
                builder.create_function(FunctionKind::Ordinary, FunctionMode::Normal, entry);
            let unreachable =
                builder.create_function(FunctionKind::Ordinary, FunctionMode::Normal, entry);
            let unreachable_binding =
                builder.create_binding(unreachable, "local", BindingKind::Let);

            builder.function_builder(entry).append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::CreateFunction(CreateFunctionOp::new(reachable)),
                [],
                UnwindTarget::Propagate,
            );

            (reachable, unreachable, unreachable_binding)
        };

        assert_eq!(prune_unreachable_functions(&mut module), 1);
        assert!(module.function(reachable).is_some());
        assert!(module.function(unreachable).is_none());
        assert!(module.binding(unreachable_binding).is_none());
        assert_eq!(prune_unreachable_functions(&mut module), 0);
    }
}
