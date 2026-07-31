//! Contextual safety queries for IR operations.

use evrel_js_ir::{
    BinaryOperator, JsFunctionIr, OperationData, OperationId, OperationKind, UnaryOperator,
};

use super::{FunctionValueAnalysis, ValueTypeSet};

/// Returns whether evaluating an operation can be safely replaced or discarded
/// without removing observable JavaScript behavior.
///
/// This query considers only the operation's evaluation. The caller remains
/// responsible for preserving required results, control flow, and operand
/// evaluation.
pub fn is_safe_to_remove(
    function: &JsFunctionIr,
    values: &FunctionValueAnalysis,
    operation: OperationId,
) -> bool {
    let Some(data) = function.operation(operation) else {
        return false;
    };
    let Some(effects) = function.operation_effects(operation) else {
        return false;
    };

    if effects.is_empty() {
        return true;
    }

    if effects.may_suspend() || effects.may_have_observable_effects() {
        return false;
    }

    match data.kind() {
        OperationKind::Unary(operation)
            if matches!(
                operation.operator(),
                UnaryOperator::Plus | UnaryOperator::Negate | UnaryOperator::BitwiseNot
            ) =>
        {
            operands_are_definitely(data, values, 1, ValueTypeSet::NUMBER)
        }

        OperationKind::Binary(operation)
            if matches!(
                operation.operator(),
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Remainder
                    | BinaryOperator::Exponentiate
                    | BinaryOperator::LooseEqual
                    | BinaryOperator::LooseNotEqual
                    | BinaryOperator::LessThan
                    | BinaryOperator::LessThanOrEqual
                    | BinaryOperator::GreaterThan
                    | BinaryOperator::GreaterThanOrEqual
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::UnsignedShiftRight
                    | BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseXor
                    | BinaryOperator::BitwiseAnd
            ) =>
        {
            operands_are_definitely(data, values, 2, ValueTypeSet::NUMBER)
        }

        _ => false,
    }
}

fn operands_are_definitely(
    operation: &OperationData,
    values: &FunctionValueAnalysis,
    expected_count: usize,
    allowed: ValueTypeSet,
) -> bool {
    operation.operands().len() == expected_count
        && operation
            .operands()
            .iter()
            .all(|operand| values.value(*operand).is_definitely(allowed))
}

#[cfg(test)]
mod tests {
    use evrel_js_ir::{
        BinaryOp, BinaryOperator, ConstantOp, ConstantValue, DebuggerOp, JsModuleIr, LoadGlobalOp,
        ModuleBuilder, OperationKind, ReturnOp, UnwindTarget, ValueId,
    };

    use super::is_safe_to_remove;
    use crate::analysis::FunctionValueAnalysis;

    #[test]
    fn accepts_numeric_addition() {
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
                UnwindTarget::Propagate,
            );
            let result = builder.operation_results(addition)[0];

            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [result],
                UnwindTarget::Propagate,
            );

            addition
        };

        let function = module.function(function).unwrap();
        let values = FunctionValueAnalysis::compute(function).unwrap();

        assert!(is_safe_to_remove(function, &values, addition));
    }

    #[test]
    fn rejects_addition_with_an_unknown_operand() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let addition = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);

            let left = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::LoadGlobal(LoadGlobalOp::new("value")),
                [],
                UnwindTarget::Propagate,
            );
            let left = builder.operation_results(left)[0];
            let right = append_number(&mut builder, 1.0);
            let addition = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Binary(BinaryOp::new(BinaryOperator::Add)),
                [left, right],
                UnwindTarget::Propagate,
            );
            let result = builder.operation_results(addition)[0];

            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [result],
                UnwindTarget::Propagate,
            );

            addition
        };

        let function = module.function(function).unwrap();
        let values = FunctionValueAnalysis::compute(function).unwrap();

        assert!(!is_safe_to_remove(function, &values, addition));
    }

    #[test]
    fn rejects_observable_operations() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let debugger = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);

            let debugger = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Debugger(DebuggerOp::new()),
                [],
                UnwindTarget::Propagate,
            );
            let result = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
                [],
                UnwindTarget::Propagate,
            );
            let result = builder.operation_results(result)[0];

            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [result],
                UnwindTarget::Propagate,
            );

            debugger
        };

        let function = module.function(function).unwrap();
        let values = FunctionValueAnalysis::compute(function).unwrap();

        assert!(!is_safe_to_remove(function, &values, debugger));
    }

    fn append_number(builder: &mut evrel_js_ir::FunctionBuilder<'_>, value: f64) -> ValueId {
        let operation = builder.append_operation(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::Constant(ConstantOp::new(ConstantValue::Number(value))),
            [],
            UnwindTarget::Propagate,
        );

        builder.operation_results(operation)[0]
    }
}
