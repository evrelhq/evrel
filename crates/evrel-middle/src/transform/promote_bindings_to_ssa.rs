//! Promotion of eligible JavaScript bindings to SSA values.

use evrel_js_ir::{BindingId, FunctionEditor, JsFunctionIr, JsModuleIr, OperationKind};

use crate::analysis::{
    ModuleBindingPromotion, PromotableBinding, RegionControlFlowGraph, RegionDominanceFrontier,
    RegionDominatorTree,
};

use super::ssa_updater::{SsaUpdate, SsaUpdater};

/// Promotes eligible binding storage to direct SSA value flow.
///
/// Returns the number of promoted bindings.
pub fn promote_bindings_to_ssa(module: &mut JsModuleIr) -> usize {
    let promotion = ModuleBindingPromotion::compute(module);
    let mut promoted = 0;

    for (function_id, function_promotion) in promotion.functions() {
        if function_promotion.is_empty() {
            continue;
        }

        // Promotion changes values and block parameters but never changes
        // block successors, so these analyses remain valid for every binding
        // promoted within this function.
        let graph = {
            let function = module
                .function(function_id)
                .expect("promotion function must remain live");

            RegionControlFlowGraph::compute(function, function.body_region())
                .expect("promotability analysis must reject unsupported control flow")
        };

        let dominance = RegionDominatorTree::compute(&graph);
        let frontier = RegionDominanceFrontier::compute(&graph, &dominance);

        let bindings = function_promotion.promotable_bindings().collect::<Vec<_>>();

        for binding in bindings {
            let promotable = function_promotion
                .promotable_binding(binding)
                .expect("promotable binding must remain present");

            // Build each update from the latest function state. An earlier
            // promotion may have rewritten an operand consumed by this
            // binding.
            let (update, operations) = {
                let function = module
                    .function(function_id)
                    .expect("promotion function must remain live");

                let update =
                    build_update(function, &graph, &dominance, &frontier, binding, promotable);

                let operations = promotable.operations().collect::<Vec<_>>();

                (update, operations)
            };

            let function = module
                .function_mut(function_id)
                .expect("promotion function must remain live");
            let mut editor = FunctionEditor::new(function);

            update.apply(&mut editor);
            editor.remove_operations(operations);

            promoted += 1;
        }
    }

    promoted
}

fn build_update(
    function: &JsFunctionIr,
    graph: &RegionControlFlowGraph,
    dominance: &RegionDominatorTree,
    frontier: &RegionDominanceFrontier,
    binding: BindingId,
    promotable: &PromotableBinding,
) -> SsaUpdate {
    let initialization = promotable.initialization();
    let initialization_data = function
        .operation(initialization)
        .expect("binding initialization must remain live");

    let OperationKind::InitializeBinding(initialize) = initialization_data.kind() else {
        panic!("promotable initialization must initialize a binding");
    };

    assert_eq!(
        initialize.binding(),
        binding,
        "promotable initialization must reference its binding",
    );

    let mut updater = SsaUpdater::new(function, graph, dominance, frontier, None);

    updater.add_definition(initialization, initialization_data.operands()[0]);

    for &store in promotable.stores() {
        let store_data = function
            .operation(store)
            .expect("binding store must remain live");

        let OperationKind::StoreBinding(store_operation) = store_data.kind() else {
            panic!("promotable store must store a binding");
        };

        assert_eq!(
            store_operation.binding(),
            binding,
            "promotable store must reference its binding",
        );

        updater.add_definition(store, store_data.operands()[0]);
    }

    for &load in promotable.loads() {
        let load_data = function
            .operation(load)
            .expect("binding load must remain live");

        let OperationKind::LoadBinding(load_operation) = load_data.kind() else {
            panic!("promotable load must load a binding");
        };

        assert_eq!(
            load_operation.binding(),
            binding,
            "promotable load must reference its binding",
        );

        updater.add_use(load, load_data.results()[0]);
    }

    updater
        .finish()
        .expect("promotability analysis must guarantee reaching values")
}

#[cfg(test)]
mod tests {
    use evrel_js_ir::{
        BinaryOp, BinaryOperator, BindingKind, BlockParameterSource, BlockTarget, ConstantOp,
        ConstantValue, FunctionKind, FunctionMode, IfOp, InitializeBindingOp, JsModuleIr, JumpOp,
        LoadBindingOp, ModuleBuilder, OperationKind, ReturnOp, StoreBindingOp, UnwindTarget,
    };

    use super::promote_bindings_to_ssa;

    #[test]
    fn promotes_a_straight_line_binding() {
        let mut module = JsModuleIr::new();
        let function_id = create_ordinary_function(&mut module);

        let (initialization, store, load, stored_value, return_operation) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let binding = module_builder.create_binding(function_id, "value", BindingKind::Var);
            let mut builder = module_builder.function_builder(function_id);

            let initial_value = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(1.0))),
                [],
                UnwindTarget::Propagate,
            );
            let initial_value = builder.operation_results(initial_value)[0];
            let initialization = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::InitializeBinding(InitializeBindingOp::new(binding)),
                [initial_value],
                UnwindTarget::Propagate,
            );

            let stored_value = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(2.0))),
                [],
                UnwindTarget::Propagate,
            );
            let stored_value = builder.operation_results(stored_value)[0];
            let store = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::StoreBinding(StoreBindingOp::new(binding)),
                [stored_value],
                UnwindTarget::Propagate,
            );

            let load = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::LoadBinding(LoadBindingOp::new(binding)),
                [],
                UnwindTarget::Propagate,
            );
            let loaded_value = builder.operation_results(load)[0];
            let return_operation = builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [loaded_value],
                UnwindTarget::Propagate,
            );

            (initialization, store, load, stored_value, return_operation)
        };

        assert_eq!(promote_bindings_to_ssa(&mut module), 1);

        let function = module.function(function_id).unwrap();

        assert!(function.operation(initialization).is_none());
        assert!(function.operation(store).is_none());
        assert!(function.operation(load).is_none());
        assert_eq!(
            function.operation(return_operation).unwrap().operands(),
            [stored_value],
        );
    }

    #[test]
    fn promotes_a_binding_across_a_diamond() {
        let mut module = JsModuleIr::new();
        let function_id = create_ordinary_function(&mut module);

        let (join, then_value, else_value, then_jump, else_jump, return_operation) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let binding = module_builder.create_binding(function_id, "value", BindingKind::Var);
            let mut builder = module_builder.function_builder(function_id);
            let then_block = builder.create_block();
            let else_block = builder.create_block();
            let join = builder.create_block();

            let initial = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
                [],
                UnwindTarget::Propagate,
            );
            let initial = builder.operation_results(initial)[0];
            builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::InitializeBinding(InitializeBindingOp::new(binding)),
                [initial],
                UnwindTarget::Propagate,
            );

            let condition = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Boolean(true))),
                [],
                UnwindTarget::Propagate,
            );
            let condition = builder.operation_results(condition)[0];
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::If(IfOp::new(
                    BlockTarget::new(then_block, 0),
                    BlockTarget::new(else_block, 0),
                    join,
                )),
                [condition],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(then_block);
            let then_value = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(1.0))),
                [],
                UnwindTarget::Propagate,
            );
            let then_value = builder.operation_results(then_value)[0];
            builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::StoreBinding(StoreBindingOp::new(binding)),
                [then_value],
                UnwindTarget::Propagate,
            );
            let then_jump = builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(join, 0))),
                [],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(else_block);
            let else_value = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(2.0))),
                [],
                UnwindTarget::Propagate,
            );
            let else_value = builder.operation_results(else_value)[0];
            builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::StoreBinding(StoreBindingOp::new(binding)),
                [else_value],
                UnwindTarget::Propagate,
            );
            let else_jump = builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(join, 0))),
                [],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(join);
            let load = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::LoadBinding(LoadBindingOp::new(binding)),
                [],
                UnwindTarget::Propagate,
            );
            let loaded_value = builder.operation_results(load)[0];
            let return_operation = builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [loaded_value],
                UnwindTarget::Propagate,
            );

            (
                join,
                then_value,
                else_value,
                then_jump,
                else_jump,
                return_operation,
            )
        };

        assert_eq!(promote_bindings_to_ssa(&mut module), 1);

        let function = module.function(function_id).unwrap();
        let [parameter] = function.block(join).unwrap().parameters() else {
            panic!("the join must receive the promoted binding value");
        };

        assert_eq!(parameter.source(), BlockParameterSource::Forwarded);
        assert_eq!(
            function.operation(then_jump).unwrap().operands(),
            [then_value],
        );
        assert_eq!(
            function.operation(else_jump).unwrap().operands(),
            [else_value],
        );
        assert_eq!(
            function.operation(return_operation).unwrap().operands(),
            [parameter.value()],
        );
        assert_no_binding_operations(function);
    }

    #[test]
    fn promotes_a_loop_carried_binding() {
        let mut module = JsModuleIr::new();
        let function_id = create_ordinary_function(&mut module);

        let (
            header,
            initial,
            incremented,
            entry_jump,
            branch,
            addition,
            backedge,
            return_operation,
        ) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let binding = module_builder.create_binding(function_id, "counter", BindingKind::Var);
            let mut builder = module_builder.function_builder(function_id);
            let header = builder.create_block();
            let body = builder.create_block();
            let exit = builder.create_block();

            let initial = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(0.0))),
                [],
                UnwindTarget::Propagate,
            );
            let initial = builder.operation_results(initial)[0];
            builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::InitializeBinding(InitializeBindingOp::new(binding)),
                [initial],
                UnwindTarget::Propagate,
            );
            let entry_jump = builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(header, 0))),
                [],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(header);
            let header_load = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::LoadBinding(LoadBindingOp::new(binding)),
                [],
                UnwindTarget::Propagate,
            );
            let header_value = builder.operation_results(header_load)[0];
            let branch = builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::If(IfOp::new(
                    BlockTarget::new(body, 0),
                    BlockTarget::new(exit, 0),
                    exit,
                )),
                [header_value],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(body);
            let body_load = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::LoadBinding(LoadBindingOp::new(binding)),
                [],
                UnwindTarget::Propagate,
            );
            let body_value = builder.operation_results(body_load)[0];
            let one = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(1.0))),
                [],
                UnwindTarget::Propagate,
            );
            let one = builder.operation_results(one)[0];
            let addition = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Binary(BinaryOp::new(BinaryOperator::Add)),
                [body_value, one],
                UnwindTarget::Propagate,
            );
            let incremented = builder.operation_results(addition)[0];
            builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::StoreBinding(StoreBindingOp::new(binding)),
                [incremented],
                UnwindTarget::Propagate,
            );
            let backedge = builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(header, 0))),
                [],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(exit);
            let exit_load = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::LoadBinding(LoadBindingOp::new(binding)),
                [],
                UnwindTarget::Propagate,
            );
            let exit_value = builder.operation_results(exit_load)[0];
            let return_operation = builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [exit_value],
                UnwindTarget::Propagate,
            );

            (
                header,
                initial,
                incremented,
                entry_jump,
                branch,
                addition,
                backedge,
                return_operation,
            )
        };

        assert_eq!(promote_bindings_to_ssa(&mut module), 1);

        let function = module.function(function_id).unwrap();
        let [parameter] = function.block(header).unwrap().parameters() else {
            panic!("the loop header must receive the loop-carried value");
        };

        assert_eq!(parameter.source(), BlockParameterSource::Forwarded);
        assert_eq!(
            function.operation(entry_jump).unwrap().operands(),
            [initial],
        );
        assert_eq!(
            function.operation(backedge).unwrap().operands(),
            [incremented],
        );
        assert_eq!(
            function.operation(branch).unwrap().operands(),
            [parameter.value()],
        );
        assert_eq!(
            function.operation(addition).unwrap().operands()[0],
            parameter.value(),
        );
        assert_eq!(
            function.operation(return_operation).unwrap().operands(),
            [parameter.value()],
        );
        assert_no_binding_operations(function);
    }

    #[test]
    fn replans_dependent_bindings_after_each_promotion() {
        let mut module = JsModuleIr::new();
        let function_id = create_ordinary_function(&mut module);

        let (initial, first_load, second_initialization, second_load, return_operation) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let first = module_builder.create_binding(function_id, "first", BindingKind::Var);
            let second = module_builder.create_binding(function_id, "second", BindingKind::Var);
            let mut builder = module_builder.function_builder(function_id);

            let initial = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(1.0))),
                [],
                UnwindTarget::Propagate,
            );
            let initial = builder.operation_results(initial)[0];
            builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::InitializeBinding(InitializeBindingOp::new(first)),
                [initial],
                UnwindTarget::Propagate,
            );

            let first_load = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::LoadBinding(LoadBindingOp::new(first)),
                [],
                UnwindTarget::Propagate,
            );
            let first_value = builder.operation_results(first_load)[0];
            let second_initialization = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::InitializeBinding(InitializeBindingOp::new(second)),
                [first_value],
                UnwindTarget::Propagate,
            );

            let second_load = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::LoadBinding(LoadBindingOp::new(second)),
                [],
                UnwindTarget::Propagate,
            );
            let second_value = builder.operation_results(second_load)[0];
            let return_operation = builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [second_value],
                UnwindTarget::Propagate,
            );

            (
                initial,
                first_load,
                second_initialization,
                second_load,
                return_operation,
            )
        };

        assert_eq!(promote_bindings_to_ssa(&mut module), 2);

        let function = module.function(function_id).unwrap();

        assert!(function.operation(first_load).is_none());
        assert!(function.operation(second_initialization).is_none());
        assert!(function.operation(second_load).is_none());
        assert_eq!(
            function.operation(return_operation).unwrap().operands(),
            [initial],
        );
        assert_no_binding_operations(function);
    }

    fn assert_no_binding_operations(function: &evrel_js_ir::JsFunctionIr) {
        assert!(function.operations().all(|(_, operation)| {
            !matches!(
                operation.kind(),
                OperationKind::InitializeBinding(_)
                    | OperationKind::LoadBinding(_)
                    | OperationKind::StoreBinding(_)
            )
        }));
    }

    fn create_ordinary_function(module: &mut JsModuleIr) -> evrel_js_ir::FunctionId {
        let entry = module.entry_function();

        ModuleBuilder::new(module).create_function(
            FunctionKind::Ordinary,
            FunctionMode::Normal,
            entry,
        )
    }
}
