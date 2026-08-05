use evrel_frontend::lower_source_file;
use evrel_js_ir::{
    BinaryOp, BinaryOperator, BlockParameterSource, BlockTarget, ConstantOp, ConstantValue, IfOp,
    JsModuleIr, JumpOp, LoadGlobalOp, ModuleBuilder, OperationKind, ReturnOp, UnaryOp,
    UnaryOperator, ValueId,
};

use super::propagate_constants;

#[test]
fn replaces_a_proven_effect_free_result_with_a_constant() {
    let mut module = JsModuleIr::new();
    let function = module.entry_function();

    let operation = {
        let mut module_builder = ModuleBuilder::new(&mut module);
        let mut builder = module_builder.function_builder(function);
        let operand = append_boolean(&mut builder, true);
        let operation = builder.append_operation(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::Unary(UnaryOp::new(UnaryOperator::LogicalNot)),
            [operand],
        );
        let result = builder.operation_results(operation)[0];

        builder.terminate(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::Return(ReturnOp::new()),
            [result],
        );

        operation
    };

    assert_eq!(
        propagate_constants(module.function_mut(function).unwrap()),
        1
    );

    let function = module.function(function).unwrap();
    let OperationKind::Constant(constant) = function.operation(operation).unwrap().kind() else {
        panic!("logical-not operation should have been replaced");
    };

    assert_eq!(constant.value(), &ConstantValue::Boolean(false));
}

#[test]
fn propagates_through_an_ssa_block_parameter_before_rewriting() {
    let mut module = JsModuleIr::new();
    let function = module.entry_function();

    let operation = {
        let mut module_builder = ModuleBuilder::new(&mut module);
        let mut builder = module_builder.function_builder(function);
        let then_block = builder.create_block();
        let else_block = builder.create_block();
        let join_block = builder.create_block();
        let joined = builder.append_block_parameter(
            join_block,
            BlockParameterSource::Forwarded,
            evrel_js_ir::ValueType::JsValue,
        );

        let condition = builder.append_operation(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::LoadGlobal(LoadGlobalOp::new("condition")),
            [],
        );
        let condition = builder.operation_results(condition)[0];
        builder.terminate(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::If(IfOp::new(
                BlockTarget::new(then_block, 0),
                BlockTarget::new(else_block, 0),
                join_block,
            )),
            [condition],
        );

        builder.switch_to_block(then_block);
        let then_value = append_boolean(&mut builder, true);
        builder.terminate(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::Jump(JumpOp::new(BlockTarget::new(join_block, 1))),
            [then_value],
        );

        builder.switch_to_block(else_block);
        let else_value = append_boolean(&mut builder, true);
        builder.terminate(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::Jump(JumpOp::new(BlockTarget::new(join_block, 1))),
            [else_value],
        );

        builder.switch_to_block(join_block);
        let operation = builder.append_operation(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::Unary(UnaryOp::new(UnaryOperator::LogicalNot)),
            [joined],
        );
        let result = builder.operation_results(operation)[0];
        builder.terminate(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::Return(ReturnOp::new()),
            [result],
        );

        operation
    };

    assert_eq!(
        propagate_constants(module.function_mut(function).unwrap()),
        1
    );

    let function = module.function(function).unwrap();
    let OperationKind::Constant(constant) = function.operation(operation).unwrap().kind() else {
        panic!("SSA consumer should have been replaced");
    };

    assert_eq!(constant.value(), &ConstantValue::Boolean(false));
}

#[test]
fn replaces_proven_non_throwing_numeric_addition() {
    let mut module = JsModuleIr::new();
    let function = module.entry_function();

    let addition = {
        let mut module_builder = ModuleBuilder::new(&mut module);
        let mut builder = module_builder.function_builder(function);
        let left = append_number(&mut builder, 20.0);
        let right = append_number(&mut builder, 22.0);
        let addition = builder.append_operation(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::Binary(BinaryOp::new(BinaryOperator::Add)),
            [left, right],
        );
        let result = builder.operation_results(addition)[0];

        builder.terminate(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::Return(ReturnOp::new()),
            [result],
        );

        addition
    };

    assert_eq!(
        propagate_constants(module.function_mut(function).unwrap()),
        1
    );

    let function = module.function(function).unwrap();
    let OperationKind::Constant(constant) = function.operation(addition).unwrap().kind() else {
        panic!("numeric addition should have been replaced");
    };

    assert_eq!(constant.value(), &ConstantValue::Number(42.0));
}

#[test]
fn ignores_existing_constants_and_reaches_a_fixed_point() {
    let mut module = JsModuleIr::new();
    let function = module.entry_function();

    {
        let mut module_builder = ModuleBuilder::new(&mut module);
        let mut builder = module_builder.function_builder(function);
        let operand = append_boolean(&mut builder, false);
        let operation = builder.append_operation(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::Unary(UnaryOp::new(UnaryOperator::LogicalNot)),
            [operand],
        );
        let result = builder.operation_results(operation)[0];

        builder.terminate(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::Return(ReturnOp::new()),
            [result],
        );
    }

    assert_eq!(
        propagate_constants(module.function_mut(function).unwrap()),
        1
    );
    assert_eq!(
        propagate_constants(module.function_mut(function).unwrap()),
        0
    );
}

#[test]
fn optimizes_a_function_with_exceptions_lifted_from_an_inline_region() {
    let mut module = lower_source_file(
        "input.js",
        "try { !true; [possiblyMissing]; } catch (error) {}",
    )
    .unwrap();
    let function = module.entry_function();

    assert_eq!(
        propagate_constants(module.function_mut(function).unwrap()),
        1
    );
}

#[test]
fn optimizes_across_explicit_finally_completion_flow() {
    let mut module = lower_source_file(
        "input.js",
        "try { !true; possiblyMissing; } finally { cleanup(); }",
    )
    .unwrap();
    let function = module.entry_function();

    assert_eq!(
        propagate_constants(module.function_mut(function).unwrap()),
        1
    );
}

fn append_boolean(builder: &mut evrel_js_ir::FunctionBuilder<'_>, value: bool) -> ValueId {
    let operation = builder.append_operation(
        evrel_js_ir::LocationId::UNKNOWN,
        OperationKind::Constant(ConstantOp::new(ConstantValue::Boolean(value))),
        [],
    );

    builder.operation_results(operation)[0]
}

fn append_number(builder: &mut evrel_js_ir::FunctionBuilder<'_>, value: f64) -> ValueId {
    let operation = builder.append_operation(
        evrel_js_ir::LocationId::UNKNOWN,
        OperationKind::Constant(ConstantOp::new(ConstantValue::Number(value))),
        [],
    );

    builder.operation_results(operation)[0]
}
