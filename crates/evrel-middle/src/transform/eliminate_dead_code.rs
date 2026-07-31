//! Worklist elimination of unused, unobservable operations.

use evrel_js_ir::{FunctionEditor, JsFunctionIr, OperationId, ValueDefinition};
use rustc_hash::FxHashSet;

use crate::analysis::{FunctionValueAnalysis, is_safe_to_remove};
use crate::work_queue::WorkQueue;

/// Removes unused operations whose evaluation is proven unobservable.
///
/// Returns zero when the function's value flow cannot be modeled soundly.
pub fn eliminate_dead_code(function: &mut JsFunctionIr) -> usize {
    let removals = {
        let Ok(values) = FunctionValueAnalysis::compute(function) else {
            return 0;
        };

        plan_dead_operations(function, &values)
    };

    let removed = removals.len();
    FunctionEditor::new(function).remove_operations(removals);
    removed
}

fn plan_dead_operations(
    function: &JsFunctionIr,
    values: &FunctionValueAnalysis,
) -> Vec<OperationId> {
    let mut work = WorkQueue::new();

    for (operation, _) in function.operations() {
        work.push(operation);
    }

    let mut planned = FxHashSet::default();
    let mut removals = Vec::new();

    while let Some(operation) = work.pop() {
        if planned.contains(&operation) || !is_dead(function, values, operation, &planned) {
            continue;
        }

        planned.insert(operation);
        removals.push(operation);

        let data = function
            .operation(operation)
            .expect("planned operation must remain live");

        for operand in data.operands() {
            let value = function
                .value(*operand)
                .expect("operation operand must remain live");

            let ValueDefinition::OperationResult {
                operation: producer,
                ..
            } = value.definition()
            else {
                continue;
            };

            work.push(*producer);
        }
    }

    removals
}

fn is_dead(
    function: &JsFunctionIr,
    values: &FunctionValueAnalysis,
    operation: OperationId,
    planned: &FxHashSet<OperationId>,
) -> bool {
    let Some(data) = function.operation(operation) else {
        return false;
    };
    let Some(block) = function.block(data.block()) else {
        return false;
    };

    if block.terminator() == Some(operation) || !data.regions().is_empty() {
        return false;
    }

    data.results().iter().all(|result| {
        function
            .value(*result)
            .expect("operation result must remain live")
            .uses()
            .iter()
            .all(|use_site| planned.contains(&use_site.operation()))
    }) && is_safe_to_remove(function, values, operation)
}

#[cfg(test)]
mod tests {
    use evrel_js_ir::{
        BinaryOp, BinaryOperator, ConstantOp, ConstantValue, DebuggerOp, JsModuleIr, LoadGlobalOp,
        ModuleBuilder, OperationKind, ReturnOp, UnwindTarget, ValueId,
    };

    use super::eliminate_dead_code;

    #[test]
    fn removes_dead_producer_chains_to_a_fixed_point() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let (left, right, addition) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);

            let left = append_number_operation(&mut builder, 20.0);
            let right = append_number_operation(&mut builder, 22.0);
            let left_value = builder.operation_results(left)[0];
            let right_value = builder.operation_results(right)[0];

            let addition = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Binary(BinaryOp::new(BinaryOperator::Add)),
                [left_value, right_value],
                UnwindTarget::Propagate,
            );

            let returned = append_undefined(&mut builder);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [returned],
                UnwindTarget::Propagate,
            );

            (left, right, addition)
        };

        assert_eq!(
            eliminate_dead_code(module.function_mut(function).unwrap()),
            3
        );
        assert_eq!(
            eliminate_dead_code(module.function_mut(function).unwrap()),
            0
        );

        let function = module.function(function).unwrap();

        assert!(function.operation(left).is_none());
        assert!(function.operation(right).is_none());
        assert!(function.operation(addition).is_none());
    }

    #[test]
    fn preserves_observable_operations() {
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

            let returned = append_undefined(&mut builder);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [returned],
                UnwindTarget::Propagate,
            );

            debugger
        };

        assert_eq!(
            eliminate_dead_code(module.function_mut(function).unwrap()),
            0
        );
        assert!(
            module
                .function(function)
                .unwrap()
                .operation(debugger)
                .is_some()
        );
    }

    #[test]
    fn preserves_an_unused_operation_that_may_throw() {
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

            let returned = append_undefined(&mut builder);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [returned],
                UnwindTarget::Propagate,
            );

            addition
        };

        assert_eq!(
            eliminate_dead_code(module.function_mut(function).unwrap()),
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

    fn append_number_operation(
        builder: &mut evrel_js_ir::FunctionBuilder<'_>,
        value: f64,
    ) -> evrel_js_ir::OperationId {
        builder.append_operation(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::Constant(ConstantOp::new(ConstantValue::Number(value))),
            [],
            UnwindTarget::Propagate,
        )
    }

    fn append_number(builder: &mut evrel_js_ir::FunctionBuilder<'_>, value: f64) -> ValueId {
        let operation = append_number_operation(builder, value);

        builder.operation_results(operation)[0]
    }

    fn append_undefined(builder: &mut evrel_js_ir::FunctionBuilder<'_>) -> ValueId {
        let operation = builder.append_operation(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
            [],
            UnwindTarget::Propagate,
        );

        builder.operation_results(operation)[0]
    }
}
