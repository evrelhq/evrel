//! Worklist-driven simplification of local JavaScript operations.

use evrel_js_ir::{
    BinaryOperator, ConstantValue, FunctionEditor, JsFunctionIr, OperationId, OperationKind,
    UnaryOperator, ValueDefinition, ValueId,
};

use crate::js::analysis::{AbstractValue, FunctionValueAnalysis, ValueTypeSet, is_safe_to_remove};
use crate::js::work_queue::WorkQueue;

/// Simplifies operations using locally provable JavaScript identities.
///
/// Constant evaluation belongs to constant propagation. Expression-tree
/// regrouping belongs to reassociation. This pass only applies bounded local
/// rewrites and immediately revisits affected users.
///
pub fn simplify_operations(function: &mut JsFunctionIr) -> usize {
    let values = FunctionValueAnalysis::analyze(function);

    let mut work = WorkQueue::new();

    for (operation, _) in function.operations() {
        work.push(operation);
    }

    let mut simplified = 0;
    let mut editor = FunctionEditor::new(function);

    while let Some(operation) = work.pop() {
        let Some(rewrite) = plan_rewrite(editor.ir(), &values, operation) else {
            continue;
        };

        let users = result_users(editor.ir(), operation);
        let result = single_result(editor.ir(), operation)
            .expect("a planned simplification must have one result");

        match rewrite {
            Rewrite::ReplaceWithValue(replacement) => {
                editor.replace_all_uses(result, replacement);
                editor.remove_operations([operation]);
            }

            Rewrite::ReplaceWithConstant(constant) => {
                editor.replace_operation_with_constant(operation, constant);
            }
        }

        simplified += 1;

        for user in users {
            work.push(user);
        }
    }

    simplified
}

#[derive(Debug)]
enum Rewrite {
    ReplaceWithValue(ValueId),
    ReplaceWithConstant(ConstantValue),
}

fn plan_rewrite(
    function: &JsFunctionIr,
    values: &FunctionValueAnalysis,
    operation: OperationId,
) -> Option<Rewrite> {
    let data = function.operation(operation)?;

    if data.results().len() != 1 || !data.regions().is_empty() {
        return None;
    }

    if !is_safe_to_remove(function, values, operation) {
        return None;
    }

    match data.kind() {
        OperationKind::Unary(unary) => {
            let [operand] = data.operands() else {
                return None;
            };

            plan_unary(function, values, unary.operator(), *operand)
        }

        OperationKind::Binary(binary) => {
            let [left, right] = data.operands() else {
                return None;
            };

            plan_binary(function, values, binary.operator(), *left, *right)
        }

        _ => None,
    }
}

fn plan_unary(
    function: &JsFunctionIr,
    values: &FunctionValueAnalysis,
    operator: UnaryOperator,
    operand: ValueId,
) -> Option<Rewrite> {
    match operator {
        UnaryOperator::Plus if values.value(operand).is_definitely(ValueTypeSet::NUMBER) => {
            Some(Rewrite::ReplaceWithValue(operand))
        }

        UnaryOperator::LogicalNot => {
            let inner = defining_operation(function, operand)?;
            let inner_data = function.operation(inner)?;

            if !matches!(
                inner_data.kind(),
                OperationKind::Unary(unary)
                    if unary.operator() == UnaryOperator::LogicalNot
            ) {
                return None;
            }

            let [base] = inner_data.operands() else {
                return None;
            };

            values
                .value(*base)
                .is_definitely(ValueTypeSet::BOOLEAN)
                .then_some(Rewrite::ReplaceWithValue(*base))
        }

        // `void` produces an exact constant and is therefore owned by
        // constant propagation. The remaining operators have no identity
        // currently provable by the available value facts.
        UnaryOperator::Plus
        | UnaryOperator::Negate
        | UnaryOperator::BitwiseNot
        | UnaryOperator::Void => None,
    }
}

fn plan_binary(
    function: &JsFunctionIr,
    values: &FunctionValueAnalysis,
    operator: BinaryOperator,
    left: ValueId,
    right: ValueId,
) -> Option<Rewrite> {
    if matches!(
        operator,
        BinaryOperator::StrictEqual | BinaryOperator::StrictNotEqual
    ) && left == right
        && is_strictly_reflexive(values.value(left))
    {
        return Some(Rewrite::ReplaceWithConstant(ConstantValue::Boolean(
            operator == BinaryOperator::StrictEqual,
        )));
    }

    let left_value = values.value(left);
    let right_value = values.value(right);

    if left == right
        && left_value.is_definitely(ValueTypeSet::NUMBER)
        && matches!(
            operator,
            BinaryOperator::LessThan | BinaryOperator::GreaterThan
        )
    {
        return Some(Rewrite::ReplaceWithConstant(ConstantValue::Boolean(false)));
    }

    match operator {
        BinaryOperator::Add => {
            // Negative zero is the Number additive identity. Positive zero is
            // not: `-0 + 0` produces positive zero.
            if left_value.is_definitely(ValueTypeSet::NUMBER) && is_number(function, right, -0.0) {
                return Some(Rewrite::ReplaceWithValue(left));
            }

            if right_value.is_definitely(ValueTypeSet::NUMBER) && is_number(function, left, -0.0) {
                return Some(Rewrite::ReplaceWithValue(right));
            }
        }

        BinaryOperator::Subtract => {
            if left_value.is_definitely(ValueTypeSet::NUMBER) && is_number(function, right, 0.0) {
                return Some(Rewrite::ReplaceWithValue(left));
            }
        }

        BinaryOperator::Multiply => {
            if left_value.is_definitely(ValueTypeSet::NUMBER) && is_number(function, right, 1.0) {
                return Some(Rewrite::ReplaceWithValue(left));
            }

            if right_value.is_definitely(ValueTypeSet::NUMBER) && is_number(function, left, 1.0) {
                return Some(Rewrite::ReplaceWithValue(right));
            }
        }

        BinaryOperator::Divide | BinaryOperator::Exponentiate => {
            if left_value.is_definitely(ValueTypeSet::NUMBER) && is_number(function, right, 1.0) {
                return Some(Rewrite::ReplaceWithValue(left));
            }
        }

        _ => {}
    }

    None
}

fn is_strictly_reflexive(value: &AbstractValue) -> bool {
    let types = value.types();

    !types.is_empty() && !types.contains(ValueTypeSet::NUMBER)
}

fn result_users(function: &JsFunctionIr, operation: OperationId) -> Vec<OperationId> {
    let Some(result) = single_result(function, operation) else {
        return Vec::new();
    };

    function
        .value(result)
        .expect("operation result must remain live")
        .uses()
        .iter()
        .map(|use_site| use_site.operation())
        .collect()
}

fn single_result(function: &JsFunctionIr, operation: OperationId) -> Option<ValueId> {
    let [result] = function.operation(operation)?.results() else {
        return None;
    };

    Some(*result)
}

fn defining_operation(function: &JsFunctionIr, value: ValueId) -> Option<OperationId> {
    let ValueDefinition::OperationResult { operation, .. } = function.value(value)?.definition()
    else {
        return None;
    };

    Some(*operation)
}

fn constant_value(function: &JsFunctionIr, value: ValueId) -> Option<&ConstantValue> {
    let operation = defining_operation(function, value)?;
    let OperationKind::Constant(constant) = function.operation(operation)?.kind() else {
        return None;
    };

    Some(constant.value())
}

fn is_number(function: &JsFunctionIr, value: ValueId, expected: f64) -> bool {
    matches!(
        constant_value(function, value),
        Some(ConstantValue::Number(value))
            if value.to_bits() == expected.to_bits()
    )
}

#[cfg(test)]
mod tests {
    use evrel_js_ir::{
        BinaryOp, BinaryOperator, ConstantOp, ConstantValue, JsModuleIr, LoadGlobalOp,
        ModuleBuilder, OperationKind, ReturnOp, UnaryOp, UnaryOperator, ValueId,
    };

    use super::simplify_operations;

    #[test]
    fn revisits_users_after_removing_numeric_identities() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let (first, second, returned, number) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let input = append_global(&mut builder, "input");
            let number = append_unary(&mut builder, UnaryOperator::Plus, input);
            let one = append_number(&mut builder, 1.0);
            let (first, first_result) =
                append_binary(&mut builder, BinaryOperator::Multiply, number, one);
            let (second, second_result) =
                append_binary(&mut builder, BinaryOperator::Multiply, first_result, one);
            let returned = builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [second_result],
            );

            (first, second, returned, number)
        };

        assert_eq!(
            simplify_operations(module.function_mut(function).unwrap()),
            2
        );
        assert_eq!(
            simplify_operations(module.function_mut(function).unwrap()),
            0
        );

        let function = module.function(function).unwrap();

        assert!(function.operation(first).is_none());
        assert!(function.operation(second).is_none());
        assert_eq!(function.operation(returned).unwrap().operands(), [number]);
    }

    #[test]
    fn removes_double_negation_only_for_a_proven_boolean() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let (inner, outer, returned, boolean) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let left = append_global(&mut builder, "left");
            let right = append_global(&mut builder, "right");
            let (_, boolean) =
                append_binary(&mut builder, BinaryOperator::StrictEqual, left, right);
            let inner = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Unary(UnaryOp::new(UnaryOperator::LogicalNot)),
                [boolean],
            );
            let inner_result = builder.operation_results(inner)[0];
            let outer = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Unary(UnaryOp::new(UnaryOperator::LogicalNot)),
                [inner_result],
            );
            let outer_result = builder.operation_results(outer)[0];
            let returned = builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [outer_result],
            );

            (inner, outer, returned, boolean)
        };

        assert_eq!(
            simplify_operations(module.function_mut(function).unwrap()),
            1
        );

        let function = module.function(function).unwrap();

        assert!(function.operation(inner).is_some());
        assert!(function.operation(outer).is_none());
        assert_eq!(function.operation(returned).unwrap().operands(), [boolean]);
    }

    #[test]
    fn preserves_positive_zero_addition() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let addition = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let input = append_global(&mut builder, "input");
            let number = append_unary(&mut builder, UnaryOperator::Plus, input);
            let zero = append_number(&mut builder, 0.0);
            let (addition, result) = append_binary(&mut builder, BinaryOperator::Add, number, zero);

            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [result],
            );

            addition
        };

        assert_eq!(
            simplify_operations(module.function_mut(function).unwrap()),
            0
        );
        assert!(
            module
                .function(function)
                .unwrap()
                .operation(addition)
                .is_some()
        );
    }

    #[test]
    fn folds_reflexive_booleans_but_preserves_possible_nan() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let (boolean_equality, number_equality) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let left = append_global(&mut builder, "left");
            let right = append_global(&mut builder, "right");
            let (_, boolean) =
                append_binary(&mut builder, BinaryOperator::StrictEqual, left, right);
            let (boolean_equality, _) =
                append_binary(&mut builder, BinaryOperator::StrictEqual, boolean, boolean);

            let input = append_global(&mut builder, "input");
            let number = append_unary(&mut builder, UnaryOperator::Plus, input);
            let (number_equality, result) =
                append_binary(&mut builder, BinaryOperator::StrictEqual, number, number);

            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [result],
            );

            (boolean_equality, number_equality)
        };

        assert_eq!(
            simplify_operations(module.function_mut(function).unwrap()),
            1
        );

        let function = module.function(function).unwrap();

        assert!(matches!(
            function.operation(boolean_equality).unwrap().kind(),
            OperationKind::Constant(constant)
                if constant.value() == &ConstantValue::Boolean(true)
        ));
        assert!(matches!(
            function.operation(number_equality).unwrap().kind(),
            OperationKind::Binary(binary)
                if binary.operator() == BinaryOperator::StrictEqual
        ));
    }

    fn append_global(builder: &mut evrel_js_ir::FunctionBuilder<'_>, name: &str) -> ValueId {
        let operation = builder.append_operation(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::LoadGlobal(LoadGlobalOp::new(name)),
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

    fn append_unary(
        builder: &mut evrel_js_ir::FunctionBuilder<'_>,
        operator: UnaryOperator,
        operand: ValueId,
    ) -> ValueId {
        let operation = builder.append_operation(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::Unary(UnaryOp::new(operator)),
            [operand],
        );

        builder.operation_results(operation)[0]
    }

    fn append_binary(
        builder: &mut evrel_js_ir::FunctionBuilder<'_>,
        operator: BinaryOperator,
        left: ValueId,
        right: ValueId,
    ) -> (evrel_js_ir::OperationId, ValueId) {
        let operation = builder.append_operation(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::Binary(BinaryOp::new(operator)),
            [left, right],
        );

        (operation, builder.operation_results(operation)[0])
    }
}
