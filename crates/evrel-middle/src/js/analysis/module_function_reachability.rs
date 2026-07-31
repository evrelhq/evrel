//! Reachability of module-owned function bodies.

use evrel_js_ir::{FunctionId, JsModuleIr};
use rustc_hash::FxHashSet;

use crate::js::work_queue::WorkQueue;

/// Functions transitively referenced from the module entry function.
#[derive(Debug, Clone)]
pub struct ModuleFunctionReachability {
    reachable: FxHashSet<FunctionId>,
}

impl ModuleFunctionReachability {
    /// Computes function reachability through static IR references.
    pub fn compute(module: &JsModuleIr) -> Self {
        let mut reachable = FxHashSet::default();
        let mut work = WorkQueue::new();

        retain_with_parents(module, module.entry_function(), &mut reachable, &mut work);

        while let Some(function) = work.pop() {
            let function = module
                .function(function)
                .expect("reachable function must remain live");

            for (_, operation) in function.operations() {
                operation.kind().visit_referenced_functions(|referenced| {
                    assert!(
                        module.function(referenced).is_some(),
                        "operation must reference a live function"
                    );
                    retain_with_parents(module, referenced, &mut reachable, &mut work);
                });
            }
        }

        Self { reachable }
    }

    /// Returns whether the module entry can transitively reference a function.
    pub fn is_reachable(&self, function: FunctionId) -> bool {
        self.reachable.contains(&function)
    }
}

fn retain_with_parents(
    module: &JsModuleIr,
    mut function: FunctionId,
    reachable: &mut FxHashSet<FunctionId>,
    work: &mut WorkQueue<FunctionId>,
) {
    loop {
        if reachable.insert(function) {
            work.push(function);
        }

        let Some(parent) = module
            .function(function)
            .expect("retained function must remain live")
            .parent_function()
        else {
            break;
        };

        function = parent;
    }
}

#[cfg(test)]
mod tests {
    use evrel_js_ir::{
        CreateFunctionOp, FunctionKind, FunctionMode, JsModuleIr, ModuleBuilder, OperationKind,
        UnwindTarget,
    };

    use super::ModuleFunctionReachability;

    #[test]
    fn follows_transitive_function_references() {
        let mut module = JsModuleIr::new();
        let entry = module.entry_function();

        let (outer, inner, orphan) = {
            let mut builder = ModuleBuilder::new(&mut module);
            let outer =
                builder.create_function(FunctionKind::Ordinary, FunctionMode::Normal, entry);
            let inner = builder.create_function(FunctionKind::Arrow, FunctionMode::Normal, outer);
            let orphan =
                builder.create_function(FunctionKind::Ordinary, FunctionMode::Normal, entry);

            builder.function_builder(entry).append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::CreateFunction(CreateFunctionOp::new(outer)),
                [],
                UnwindTarget::Propagate,
            );
            builder.function_builder(outer).append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::CreateFunction(CreateFunctionOp::new(inner)),
                [],
                UnwindTarget::Propagate,
            );

            (outer, inner, orphan)
        };

        let reachability = ModuleFunctionReachability::compute(&module);

        assert!(reachability.is_reachable(entry));
        assert!(reachability.is_reachable(outer));
        assert!(reachability.is_reachable(inner));
        assert!(!reachability.is_reachable(orphan));
    }

    #[test]
    fn retains_lexical_parents_of_referenced_functions() {
        let mut module = JsModuleIr::new();
        let entry = module.entry_function();

        let (outer, inner) = {
            let mut builder = ModuleBuilder::new(&mut module);
            let outer =
                builder.create_function(FunctionKind::Ordinary, FunctionMode::Normal, entry);
            let inner = builder.create_function(FunctionKind::Arrow, FunctionMode::Normal, outer);

            builder.function_builder(entry).append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::CreateFunction(CreateFunctionOp::new(inner)),
                [],
                UnwindTarget::Propagate,
            );

            (outer, inner)
        };

        let reachability = ModuleFunctionReachability::compute(&module);

        assert!(reachability.is_reachable(outer));
        assert!(reachability.is_reachable(inner));
    }
}
