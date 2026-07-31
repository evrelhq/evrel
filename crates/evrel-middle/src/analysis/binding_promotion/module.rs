//! Module-wide binding-promotion eligibility.

use std::collections::{BTreeMap, BTreeSet};

use evrel_js_ir::{BindingId, BindingKind, FunctionId, JsModuleIr, OperationKind};

use super::function::{FunctionBindingPromotion, FunctionBindingPromotionBuilder};

/// Binding-promotion eligibility for every function in one module.
#[derive(Debug, Clone)]
pub struct ModuleBindingPromotion {
    functions: BTreeMap<FunctionId, FunctionBindingPromotion>,
}

impl ModuleBindingPromotion {
    /// Computes conservative binding-promotion eligibility.
    pub fn compute(module: &JsModuleIr) -> Self {
        let mut builders = module
            .functions()
            .map(|(function, _)| (function, FunctionBindingPromotionBuilder::default()))
            .collect::<BTreeMap<_, _>>();

        add_candidates(module, &mut builders);
        record_binding_references(module, &mut builders);
        reject_bindings_visible_to_eval(module, &mut builders);

        let functions = builders
            .into_iter()
            .map(|(function_id, builder)| {
                let function = module
                    .function(function_id)
                    .expect("promotion function must remain live");

                (function_id, builder.finish(function))
            })
            .collect();

        Self { functions }
    }

    /// Returns promotion eligibility for one function.
    pub fn function(&self, function: FunctionId) -> Option<&FunctionBindingPromotion> {
        self.functions.get(&function)
    }

    /// Iterates over function analyses in deterministic function-ID order.
    pub fn functions(&self) -> impl Iterator<Item = (FunctionId, &FunctionBindingPromotion)> + '_ {
        self.functions
            .iter()
            .map(|(&function, promotion)| (function, promotion))
    }
}

fn add_candidates(
    module: &JsModuleIr,
    builders: &mut BTreeMap<FunctionId, FunctionBindingPromotionBuilder>,
) {
    let exported = module
        .exports()
        .iter()
        .filter_map(|export| export.binding())
        .collect::<BTreeSet<_>>();

    for (binding, data) in module.bindings() {
        // Begin with ordinary mutable function-local storage. Other binding
        // kinds can be added without changing the transform architecture.
        if data.kind() != BindingKind::Var
            || data.declaring_function() == module.entry_function()
            || exported.contains(&binding)
        {
            continue;
        }

        builders
            .get_mut(&data.declaring_function())
            .expect("binding must be declared by a live function")
            .add_candidate(binding);
    }
}

fn record_binding_references(
    module: &JsModuleIr,
    builders: &mut BTreeMap<FunctionId, FunctionBindingPromotionBuilder>,
) {
    for (function_id, function) in module.functions() {
        for (operation_id, operation) in function.operations() {
            let mut referenced_bindings = Vec::new();

            operation.kind().visit_referenced_bindings(|binding| {
                referenced_bindings.push(binding);
            });

            for binding in referenced_bindings {
                record_binding_reference(module, builders, function_id, operation_id, binding);
            }
        }
    }
}

fn record_binding_reference(
    module: &JsModuleIr,
    builders: &mut BTreeMap<FunctionId, FunctionBindingPromotionBuilder>,
    referencing_function: FunctionId,
    operation: evrel_js_ir::OperationId,
    binding: BindingId,
) {
    let declaring_function = module
        .binding(binding)
        .expect("operation must reference a live binding")
        .declaring_function();

    let builder = builders
        .get_mut(&declaring_function)
        .expect("binding must be declared by a live function");

    if referencing_function != declaring_function {
        // A different function observes the environment cell.
        builder.reject(binding);
        return;
    }

    let function = module
        .function(referencing_function)
        .expect("referencing function must remain live");

    builder.record_reference(function, operation, binding);
}

fn reject_bindings_visible_to_eval(
    module: &JsModuleIr,
    builders: &mut BTreeMap<FunctionId, FunctionBindingPromotionBuilder>,
) {
    let eval_functions = module
        .functions()
        .filter_map(|(function_id, function)| {
            function
                .operations()
                .any(|(_, operation)| {
                    matches!(
                        operation.kind(),
                        OperationKind::LoadGlobal(global)
                            if global.name() == "eval"
                    )
                })
                .then_some(function_id)
        })
        .collect::<Vec<_>>();

    for eval_function in eval_functions {
        // Direct eval can observe bindings in its own function and every
        // lexically enclosing function.
        let mut visible_function = Some(eval_function);

        while let Some(function_id) = visible_function {
            builders
                .get_mut(&function_id)
                .expect("eval function must remain live")
                .reject_all();

            visible_function = module
                .function(function_id)
                .expect("function ancestry must remain live")
                .parent_function();
        }
    }
}

#[cfg(test)]
mod tests {
    use evrel_js_ir::{
        BindingId, BindingKind, ConstantOp, ConstantValue, FunctionId, FunctionKind, FunctionMode,
        InitializeBindingOp, JsModuleIr, LoadBindingOp, LoadGlobalOp, ModuleBuilder, ModuleExport,
        ModuleExportName, OperationKind, TextRange, UnwindTarget,
    };

    use super::ModuleBindingPromotion;

    fn initialize_with_undefined(
        module: &mut JsModuleIr,
        function: FunctionId,
        binding: BindingId,
    ) {
        let mut module_builder = ModuleBuilder::new(module);
        let mut builder = module_builder.function_builder(function);
        let undefined = builder.append_operation(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
            [],
            UnwindTarget::Propagate,
        );
        let undefined = builder.operation_results(undefined)[0];
        builder.append_operation(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::InitializeBinding(InitializeBindingOp::new(binding)),
            [undefined],
            UnwindTarget::Propagate,
        );
    }

    #[test]
    fn accepts_a_local_var_and_ignores_other_binding_kinds() {
        let mut module = JsModuleIr::new();
        let entry = module.entry_function();

        let (function, var_binding, let_binding) = {
            let mut builder = ModuleBuilder::new(&mut module);
            let function =
                builder.create_function(FunctionKind::Ordinary, FunctionMode::Normal, entry);
            (
                function,
                builder.create_binding(function, "promoted", BindingKind::Var),
                builder.create_binding(function, "retained", BindingKind::Let),
            )
        };
        initialize_with_undefined(&mut module, function, var_binding);
        initialize_with_undefined(&mut module, function, let_binding);

        let promotion = ModuleBindingPromotion::compute(&module);
        let function_promotion = promotion.function(function).unwrap();

        assert!(function_promotion.is_promotable(var_binding));
        assert!(!function_promotion.is_promotable(let_binding));
    }

    #[test]
    fn excludes_a_binding_declared_by_the_entry_function() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let binding = {
            let mut builder = ModuleBuilder::new(&mut module);
            builder.create_binding(function, "global", BindingKind::Var)
        };
        initialize_with_undefined(&mut module, function, binding);

        let promotion = ModuleBindingPromotion::compute(&module);

        assert!(!promotion.function(function).unwrap().is_promotable(binding));
    }

    #[test]
    fn excludes_an_exported_binding() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let binding = {
            let mut builder = ModuleBuilder::new(&mut module);
            let binding = builder.create_binding(function, "value", BindingKind::Var);
            let source = "export { value };";
            let file = builder.add_source_file("input.mjs", source);
            let location = builder.source_location(file, TextRange::new(0, source.len() as u32));
            builder.add_export(ModuleExport::local(
                location,
                ModuleExportName::Identifier("value".into()),
                binding,
            ));
            binding
        };
        initialize_with_undefined(&mut module, function, binding);

        let promotion = ModuleBindingPromotion::compute(&module);

        assert!(!promotion.function(function).unwrap().is_promotable(binding));
    }

    #[test]
    fn rejects_a_binding_captured_by_a_nested_function() {
        let mut module = JsModuleIr::new();
        let entry = module.entry_function();

        let (function, binding, nested) = {
            let mut builder = ModuleBuilder::new(&mut module);
            let function =
                builder.create_function(FunctionKind::Ordinary, FunctionMode::Normal, entry);
            let binding = builder.create_binding(function, "value", BindingKind::Var);
            let nested =
                builder.create_function(FunctionKind::Ordinary, FunctionMode::Normal, function);
            (function, binding, nested)
        };
        initialize_with_undefined(&mut module, function, binding);

        {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(nested);
            builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::LoadBinding(LoadBindingOp::new(binding)),
                [],
                UnwindTarget::Propagate,
            );
        }

        let promotion = ModuleBindingPromotion::compute(&module);

        assert!(!promotion.function(function).unwrap().is_promotable(binding));
    }

    #[test]
    fn rejects_bindings_visible_to_possible_direct_eval() {
        let mut module = JsModuleIr::new();
        let entry = module.entry_function();

        let (function, binding) = {
            let mut builder = ModuleBuilder::new(&mut module);
            let function =
                builder.create_function(FunctionKind::Ordinary, FunctionMode::Normal, entry);
            let binding = builder.create_binding(function, "value", BindingKind::Var);
            (function, binding)
        };
        initialize_with_undefined(&mut module, function, binding);

        {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::LoadGlobal(LoadGlobalOp::new("eval")),
                [],
                UnwindTarget::Propagate,
            );
        }

        let promotion = ModuleBindingPromotion::compute(&module);

        assert!(!promotion.function(function).unwrap().is_promotable(binding));
    }
}
