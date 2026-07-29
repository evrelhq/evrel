use evrel_ir::{
    BinaryOp, BinaryOperator, BlockParameterSource, BlockTarget, ConstantOp, ConstantValue, IfOp,
    JumpOp, LoadGlobalOp, ModuleBuilder, ModuleIr, OperationKind, ReturnOp, UnwindTarget, ValueId,
};

use super::FunctionValueInputs;
use super::sparse::SparseValueAnalysis;

#[test]
fn propagates_constants_through_straight_line_ssa() {
    let mut module = ModuleIr::new();
    let function = module.entry_function();

    let result = {
        let mut module_builder = ModuleBuilder::new(&mut module);
        let mut builder = module_builder.function_builder(function);

        let left = append_number(&mut builder, 20.0);
        let right = append_number(&mut builder, 22.0);
        let addition = builder.append_operation(
            OperationKind::Binary(BinaryOp::new(BinaryOperator::Add)),
            [left, right],
            UnwindTarget::Propagate,
        );
        let result = builder.operation_results(addition)[0];

        builder.terminate(
            OperationKind::Return(ReturnOp::new()),
            [result],
            UnwindTarget::Propagate,
        );

        result
    };

    let analysis = SparseValueAnalysis::compute(
        module.function(function).unwrap(),
        &FunctionValueInputs::new(),
    )
    .unwrap();

    assert_eq!(
        analysis.value(result).constant(),
        Some(&ConstantValue::Number(42.0)),
    );
}

#[test]
fn follows_only_the_selected_edge_of_a_constant_branch() {
    let mut module = ModuleIr::new();
    let function = module.entry_function();

    let (branch, then_block, else_block, joined) = {
        let mut module_builder = ModuleBuilder::new(&mut module);
        let mut builder = module_builder.function_builder(function);
        let then_block = builder.create_block();
        let else_block = builder.create_block();
        let join_block = builder.create_block();
        let joined = builder.append_block_parameter(join_block, BlockParameterSource::Forwarded);

        let condition = append_boolean(&mut builder, true);
        let branch = builder.terminate(
            OperationKind::If(IfOp::new(
                BlockTarget::new(then_block, 0),
                BlockTarget::new(else_block, 0),
                join_block,
            )),
            [condition],
            UnwindTarget::Propagate,
        );

        builder.switch_to_block(then_block);
        let then_value = append_number(&mut builder, 1.0);
        builder.terminate(
            OperationKind::Jump(JumpOp::new(BlockTarget::new(join_block, 1))),
            [then_value],
            UnwindTarget::Propagate,
        );

        builder.switch_to_block(else_block);
        let else_value = append_number(&mut builder, 2.0);
        builder.terminate(
            OperationKind::Jump(JumpOp::new(BlockTarget::new(join_block, 1))),
            [else_value],
            UnwindTarget::Propagate,
        );

        builder.switch_to_block(join_block);
        builder.terminate(
            OperationKind::Return(ReturnOp::new()),
            [joined],
            UnwindTarget::Propagate,
        );

        (branch, then_block, else_block, joined)
    };

    let analysis = SparseValueAnalysis::compute(
        module.function(function).unwrap(),
        &FunctionValueInputs::new(),
    )
    .unwrap();

    assert!(analysis.is_edge_executable(branch, 0));
    assert!(!analysis.is_edge_executable(branch, 1));
    assert!(analysis.is_block_executable(then_block));
    assert!(!analysis.is_block_executable(else_block));
    assert_eq!(
        analysis.value(joined).constant(),
        Some(&ConstantValue::Number(1.0)),
    );
}

#[test]
fn retains_an_equal_constant_from_multiple_executable_edges() {
    let (module, function, joined) = build_dynamic_diamond(1.0, 1.0);
    let analysis = SparseValueAnalysis::compute(
        module.function(function).unwrap(),
        &FunctionValueInputs::new(),
    )
    .unwrap();

    assert_eq!(
        analysis.value(joined).constant(),
        Some(&ConstantValue::Number(1.0)),
    );
}

#[test]
fn loses_constant_information_at_a_conflicting_join() {
    let (module, function, joined) = build_dynamic_diamond(1.0, 2.0);
    let analysis = SparseValueAnalysis::compute(
        module.function(function).unwrap(),
        &FunctionValueInputs::new(),
    )
    .unwrap();

    assert_eq!(analysis.value(joined).constant(), None);
    assert!(!analysis.value(joined).is_bottom());
}

#[test]
fn revisits_loop_users_when_a_backedge_changes_a_parameter() {
    let mut module = ModuleIr::new();
    let function = module.entry_function();

    let (carried, derived) = {
        let mut module_builder = ModuleBuilder::new(&mut module);
        let mut builder = module_builder.function_builder(function);
        let header = builder.create_block();
        let body = builder.create_block();
        let exit = builder.create_block();
        let carried = builder.append_block_parameter(header, BlockParameterSource::Forwarded);

        let initial = append_number(&mut builder, 1.0);
        let backedge = append_number(&mut builder, 2.0);
        let zero = append_number(&mut builder, 0.0);
        builder.terminate(
            OperationKind::Jump(JumpOp::new(BlockTarget::new(header, 1))),
            [initial],
            UnwindTarget::Propagate,
        );

        builder.switch_to_block(header);
        let derived = builder.append_operation(
            OperationKind::Binary(BinaryOp::new(BinaryOperator::Add)),
            [carried, zero],
            UnwindTarget::Propagate,
        );
        let derived = builder.operation_results(derived)[0];
        let condition = builder.append_operation(
            OperationKind::LoadGlobal(LoadGlobalOp::new("repeat")),
            [],
            UnwindTarget::Propagate,
        );
        let condition = builder.operation_results(condition)[0];
        builder.terminate(
            OperationKind::If(IfOp::new(
                BlockTarget::new(body, 0),
                BlockTarget::new(exit, 0),
                exit,
            )),
            [condition],
            UnwindTarget::Propagate,
        );

        builder.switch_to_block(body);
        builder.terminate(
            OperationKind::Jump(JumpOp::new(BlockTarget::new(header, 1))),
            [backedge],
            UnwindTarget::Propagate,
        );

        builder.switch_to_block(exit);
        builder.terminate(
            OperationKind::Return(ReturnOp::new()),
            [derived],
            UnwindTarget::Propagate,
        );

        (carried, derived)
    };

    let analysis = SparseValueAnalysis::compute(
        module.function(function).unwrap(),
        &FunctionValueInputs::new(),
    )
    .unwrap();

    assert_eq!(analysis.value(carried).constant(), None);
    assert!(!analysis.value(carried).is_bottom());
    assert_eq!(analysis.value(derived).constant(), None);
    assert!(!analysis.value(derived).is_bottom());
}

fn build_dynamic_diamond(
    then_number: f64,
    else_number: f64,
) -> (ModuleIr, evrel_ir::FunctionId, ValueId) {
    let mut module = ModuleIr::new();
    let function = module.entry_function();

    let joined = {
        let mut module_builder = ModuleBuilder::new(&mut module);
        let mut builder = module_builder.function_builder(function);
        let then_block = builder.create_block();
        let else_block = builder.create_block();
        let join_block = builder.create_block();
        let joined = builder.append_block_parameter(join_block, BlockParameterSource::Forwarded);

        let condition = builder.append_operation(
            OperationKind::LoadGlobal(LoadGlobalOp::new("condition")),
            [],
            UnwindTarget::Propagate,
        );
        let condition = builder.operation_results(condition)[0];
        builder.terminate(
            OperationKind::If(IfOp::new(
                BlockTarget::new(then_block, 0),
                BlockTarget::new(else_block, 0),
                join_block,
            )),
            [condition],
            UnwindTarget::Propagate,
        );

        builder.switch_to_block(then_block);
        let then_value = append_number(&mut builder, then_number);
        builder.terminate(
            OperationKind::Jump(JumpOp::new(BlockTarget::new(join_block, 1))),
            [then_value],
            UnwindTarget::Propagate,
        );

        builder.switch_to_block(else_block);
        let else_value = append_number(&mut builder, else_number);
        builder.terminate(
            OperationKind::Jump(JumpOp::new(BlockTarget::new(join_block, 1))),
            [else_value],
            UnwindTarget::Propagate,
        );

        builder.switch_to_block(join_block);
        builder.terminate(
            OperationKind::Return(ReturnOp::new()),
            [joined],
            UnwindTarget::Propagate,
        );

        joined
    };

    (module, function, joined)
}

fn append_number(builder: &mut evrel_ir::FunctionBuilder<'_>, value: f64) -> ValueId {
    let operation = builder.append_operation(
        OperationKind::Constant(ConstantOp::new(ConstantValue::Number(value))),
        [],
        UnwindTarget::Propagate,
    );

    builder.operation_results(operation)[0]
}

fn append_boolean(builder: &mut evrel_ir::FunctionBuilder<'_>, value: bool) -> ValueId {
    let operation = builder.append_operation(
        OperationKind::Constant(ConstantOp::new(ConstantValue::Boolean(value))),
        [],
        UnwindTarget::Propagate,
    );

    builder.operation_results(operation)[0]
}
