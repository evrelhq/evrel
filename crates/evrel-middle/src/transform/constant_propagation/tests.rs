use evrel_ir::{
    BinaryOp, BinaryOperator, BlockParameterSource, BlockTarget, ConstantOp, ConstantValue, IfOp,
    JumpOp, LoadGlobalOp, ModuleBuilder, ModuleIr, OperationKind, ReturnOp, UnaryOp, UnaryOperator,
    UnwindTarget, ValueId,
};

use super::propagate_constants;

#[test]
fn replaces_a_proven_effect_free_result_with_a_constant() {
    let mut module = ModuleIr::new();
    let function = module.entry_function();

    let operation = {
        let mut module_builder = ModuleBuilder::new(&mut module);
        let mut builder = module_builder.function_builder(function);
        let operand = append_boolean(&mut builder, true);
        let operation = builder.append_operation(
            OperationKind::Unary(UnaryOp::new(UnaryOperator::LogicalNot)),
            [operand],
            UnwindTarget::Propagate,
        );
        let result = builder.operation_results(operation)[0];

        builder.terminate(
            OperationKind::Return(ReturnOp::new()),
            [result],
            UnwindTarget::Propagate,
        );

        operation
    };

    assert_eq!(propagate_constants(&mut module), 1);

    let function = module.function(function).unwrap();
    let OperationKind::Constant(constant) = function.operation(operation).unwrap().kind() else {
        panic!("logical-not operation should have been replaced");
    };

    assert_eq!(constant.value(), &ConstantValue::Boolean(false));
}

#[test]
fn propagates_through_an_ssa_block_parameter_before_rewriting() {
    let mut module = ModuleIr::new();
    let function = module.entry_function();

    let operation = {
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
        let then_value = append_boolean(&mut builder, true);
        builder.terminate(
            OperationKind::Jump(JumpOp::new(BlockTarget::new(join_block, 1))),
            [then_value],
            UnwindTarget::Propagate,
        );

        builder.switch_to_block(else_block);
        let else_value = append_boolean(&mut builder, true);
        builder.terminate(
            OperationKind::Jump(JumpOp::new(BlockTarget::new(join_block, 1))),
            [else_value],
            UnwindTarget::Propagate,
        );

        builder.switch_to_block(join_block);
        let operation = builder.append_operation(
            OperationKind::Unary(UnaryOp::new(UnaryOperator::LogicalNot)),
            [joined],
            UnwindTarget::Propagate,
        );
        let result = builder.operation_results(operation)[0];
        builder.terminate(
            OperationKind::Return(ReturnOp::new()),
            [result],
            UnwindTarget::Propagate,
        );

        operation
    };

    assert_eq!(propagate_constants(&mut module), 1);

    let function = module.function(function).unwrap();
    let OperationKind::Constant(constant) = function.operation(operation).unwrap().kind() else {
        panic!("SSA consumer should have been replaced");
    };

    assert_eq!(constant.value(), &ConstantValue::Boolean(false));
}

#[test]
fn replaces_proven_non_throwing_numeric_addition() {
    let mut module = ModuleIr::new();
    let function = module.entry_function();

    let addition = {
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

        addition
    };

    assert_eq!(propagate_constants(&mut module), 1);

    let function = module.function(function).unwrap();
    let OperationKind::Constant(constant) = function.operation(addition).unwrap().kind() else {
        panic!("numeric addition should have been replaced");
    };

    assert_eq!(constant.value(), &ConstantValue::Number(42.0));
}

#[test]
fn ignores_existing_constants_and_reaches_a_fixed_point() {
    let mut module = ModuleIr::new();
    let function = module.entry_function();

    {
        let mut module_builder = ModuleBuilder::new(&mut module);
        let mut builder = module_builder.function_builder(function);
        let operand = append_boolean(&mut builder, false);
        let operation = builder.append_operation(
            OperationKind::Unary(UnaryOp::new(UnaryOperator::LogicalNot)),
            [operand],
            UnwindTarget::Propagate,
        );
        let result = builder.operation_results(operation)[0];

        builder.terminate(
            OperationKind::Return(ReturnOp::new()),
            [result],
            UnwindTarget::Propagate,
        );
    }

    assert_eq!(propagate_constants(&mut module), 1);
    assert_eq!(propagate_constants(&mut module), 0);
}

#[test]
fn skips_a_function_with_implicit_local_exception_flow() {
    let mut module = ModuleIr::new();
    let function = module.entry_function();

    let candidate = {
        let mut module_builder = ModuleBuilder::new(&mut module);
        let mut builder = module_builder.function_builder(function);
        let catch_entry = builder.create_block();
        let (handler, _) = builder.create_catch_handler(None, catch_entry);

        let operand = append_boolean(&mut builder, true);
        let candidate = builder.append_operation(
            OperationKind::Unary(UnaryOp::new(UnaryOperator::LogicalNot)),
            [operand],
            UnwindTarget::Propagate,
        );
        let candidate_result = builder.operation_results(candidate)[0];

        builder.append_operation(
            OperationKind::LoadGlobal(LoadGlobalOp::new("possiblyMissing")),
            [],
            UnwindTarget::Handler(handler),
        );
        builder.terminate(
            OperationKind::Return(ReturnOp::new()),
            [candidate_result],
            UnwindTarget::Propagate,
        );

        candidate
    };

    assert_eq!(propagate_constants(&mut module), 0);
    assert!(matches!(
        module
            .function(function)
            .unwrap()
            .operation(candidate)
            .unwrap()
            .kind(),
        OperationKind::Unary(_),
    ));
}

fn append_boolean(builder: &mut evrel_ir::FunctionBuilder<'_>, value: bool) -> ValueId {
    let operation = builder.append_operation(
        OperationKind::Constant(ConstantOp::new(ConstantValue::Boolean(value))),
        [],
        UnwindTarget::Propagate,
    );

    builder.operation_results(operation)[0]
}

fn append_number(builder: &mut evrel_ir::FunctionBuilder<'_>, value: f64) -> ValueId {
    let operation = builder.append_operation(
        OperationKind::Constant(ConstantOp::new(ConstantValue::Number(value))),
        [],
        UnwindTarget::Propagate,
    );

    builder.operation_results(operation)[0]
}
