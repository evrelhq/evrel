//! Elimination of repeated deterministic SSA expressions.

use evrel_js_ir::{
    BinaryOperator, BlockId, FunctionEditor, JsFunctionIr, OperationData, OperationId,
    OperationKind, TypeofTarget, UnaryOperator, ValueId,
};
use rustc_hash::FxHashMap;

use crate::js::analysis::{
    FunctionValueAnalysis, RegionControlFlowGraph, RegionDominatorTree, is_safe_to_remove,
};

/// Removes repeated deterministic expressions whose earlier result dominates
/// the repeated evaluation.
///
/// Returns zero when the function's value flow or control flow cannot be
/// modeled soundly.
pub fn eliminate_common_subexpressions(function: &mut JsFunctionIr) -> usize {
    let replacements = {
        let Ok(values) = FunctionValueAnalysis::compute(function) else {
            return 0;
        };

        let Some(replacements) = plan_replacements(function, &values) else {
            return 0;
        };

        replacements
    };

    apply_replacements(function, replacements)
}

/// The operation-specific part of an expression's identity.
///
/// Only deterministic value operations belong here. Allocations, memory reads,
/// calls, and other identity-sensitive or externally observable operations are
/// deliberately excluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ExpressionKind {
    IsNullish,
    TypeofValue,
    Unary(UnaryOperator),
    Binary(BinaryOperator),
}

/// The exact SSA expression computed by an operation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ExpressionKey {
    kind: ExpressionKind,
    operands: Vec<ValueId>,
}

/// One redundant operation and the dominating value that replaces its result.
#[derive(Debug, Clone, Copy)]
struct Replacement {
    operation: OperationId,
    result: ValueId,
    dominating_result: ValueId,
}

/// One step in an iterative traversal of the dominator tree.
enum Visit {
    Enter(BlockId),
    ExitScope(usize),
}

fn plan_replacements(
    function: &JsFunctionIr,
    values: &FunctionValueAnalysis,
) -> Option<Vec<Replacement>> {
    let mut replacements = Vec::new();

    for (region, _) in function.regions() {
        let graph = RegionControlFlowGraph::compute(function, region).ok()?;
        let dominators = RegionDominatorTree::compute(&graph);

        plan_region_replacements(function, values, &dominators, &mut replacements);
    }

    Some(replacements)
}

fn plan_region_replacements(
    function: &JsFunctionIr,
    values: &FunctionValueAnalysis,
    dominators: &RegionDominatorTree,
    replacements: &mut Vec<Replacement>,
) {
    let mut available = FxHashMap::<ExpressionKey, ValueId>::default();
    let mut scope = Vec::<ExpressionKey>::new();
    let mut work = vec![Visit::Enter(dominators.entry())];

    while let Some(visit) = work.pop() {
        match visit {
            Visit::Enter(block_id) => {
                let checkpoint = scope.len();
                let block = function
                    .block(block_id)
                    .expect("dominator tree must reference a live block");

                for &operation in block.operations() {
                    let data = function
                        .operation(operation)
                        .expect("block must reference a live operation");

                    let Some(key) = expression_key(data) else {
                        continue;
                    };

                    if !is_safe_to_remove(function, values, operation) {
                        continue;
                    }

                    let result = data.results()[0];

                    if let Some(&dominating_result) = available.get(&key) {
                        replacements.push(Replacement {
                            operation,
                            result,
                            dominating_result,
                        });
                    } else {
                        available.insert(key.clone(), result);
                        scope.push(key);
                    }
                }

                work.push(Visit::ExitScope(checkpoint));

                if let Some(children) = dominators.children(block_id) {
                    work.extend(children.iter().rev().copied().map(Visit::Enter));
                }
            }

            Visit::ExitScope(checkpoint) => {
                while scope.len() > checkpoint {
                    let key = scope
                        .pop()
                        .expect("scope must contain every available expression");

                    let removed = available.remove(&key);
                    debug_assert!(removed.is_some());
                }
            }
        }
    }
}

fn expression_key(operation: &OperationData) -> Option<ExpressionKey> {
    if operation.results().len() != 1 || !operation.regions().is_empty() {
        return None;
    }

    let kind = match operation.kind() {
        OperationKind::IsNullish(_) => ExpressionKind::IsNullish,

        OperationKind::Typeof(operation) if matches!(operation.target(), TypeofTarget::Value) => {
            ExpressionKind::TypeofValue
        }

        OperationKind::Unary(operation) => ExpressionKind::Unary(operation.operator()),

        OperationKind::Binary(operation) => ExpressionKind::Binary(operation.operator()),

        _ => return None,
    };

    Some(ExpressionKey {
        kind,
        operands: operation.operands().to_vec(),
    })
}

fn apply_replacements(function: &mut JsFunctionIr, replacements: Vec<Replacement>) -> usize {
    let removed = replacements.len();
    let operations = replacements
        .iter()
        .map(|replacement| replacement.operation)
        .collect::<Vec<_>>();

    let mut editor = FunctionEditor::new(function);

    for replacement in replacements {
        editor.replace_all_uses(replacement.result, replacement.dominating_result);
    }

    editor.remove_operations(operations);

    removed
}

#[cfg(test)]
mod tests {
    use evrel_js_ir::{
        BinaryOp, BinaryOperator, BlockTarget, ConstantOp, ConstantValue, IfOp, JsModuleIr, JumpOp,
        LoadGlobalOp, LoadThisOp, ModuleBuilder, OperationId, OperationKind, ReturnOp,
        UnwindTarget, ValueId,
    };

    use super::eliminate_common_subexpressions;

    #[test]
    fn removes_a_repeated_expression_in_one_block() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let (first, first_result, repeated, returned) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);

            let left = append_number(&mut builder, 20.0);
            let right = append_number(&mut builder, 22.0);
            let (first, first_result) = append_addition(&mut builder, left, right);
            let (repeated, repeated_result) = append_addition(&mut builder, left, right);

            let returned = builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [repeated_result],
                UnwindTarget::Propagate,
            );

            (first, first_result, repeated, returned)
        };

        assert_eq!(
            eliminate_common_subexpressions(module.function_mut(function).unwrap()),
            1
        );
        assert_eq!(
            eliminate_common_subexpressions(module.function_mut(function).unwrap()),
            0
        );

        let function = module.function(function).unwrap();

        assert!(function.operation(first).is_some());
        assert!(function.operation(repeated).is_none());
        assert_eq!(
            function.operation(returned).unwrap().operands(),
            [first_result],
        );
    }

    #[test]
    fn removes_an_expression_dominated_by_an_earlier_block() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let (first, first_result, repeated, returned) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let successor = builder.create_block();

            let left = append_number(&mut builder, 20.0);
            let right = append_number(&mut builder, 22.0);
            let (first, first_result) = append_addition(&mut builder, left, right);

            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(successor, 0))),
                [],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(successor);
            let (repeated, repeated_result) = append_addition(&mut builder, left, right);
            let returned = builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [repeated_result],
                UnwindTarget::Propagate,
            );

            (first, first_result, repeated, returned)
        };

        assert_eq!(
            eliminate_common_subexpressions(module.function_mut(function).unwrap()),
            1
        );
        assert_eq!(
            eliminate_common_subexpressions(module.function_mut(function).unwrap()),
            0
        );

        let function = module.function(function).unwrap();

        assert!(function.operation(first).is_some());
        assert!(function.operation(repeated).is_none());
        assert_eq!(
            function.operation(returned).unwrap().operands(),
            [first_result],
        );
    }

    #[test]
    fn preserves_expressions_from_sibling_blocks() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let (left_addition, right_addition) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let left_block = builder.create_block();
            let right_block = builder.create_block();
            let completion = builder.create_block();

            let operand_left = append_number(&mut builder, 20.0);
            let operand_right = append_number(&mut builder, 22.0);

            let condition = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::LoadThis(LoadThisOp::new()),
                [],
                UnwindTarget::Propagate,
            );
            let condition = builder.operation_results(condition)[0];

            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::If(IfOp::new(
                    BlockTarget::new(left_block, 0),
                    BlockTarget::new(right_block, 0),
                    completion,
                )),
                [condition],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(left_block);
            let (left_addition, left_result) =
                append_addition(&mut builder, operand_left, operand_right);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [left_result],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(right_block);
            let (right_addition, right_result) =
                append_addition(&mut builder, operand_left, operand_right);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [right_result],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(completion);
            let undefined = append_undefined(&mut builder);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [undefined],
                UnwindTarget::Propagate,
            );

            (left_addition, right_addition)
        };

        assert_eq!(
            eliminate_common_subexpressions(module.function_mut(function).unwrap()),
            0
        );

        let function = module.function(function).unwrap();

        assert!(function.operation(left_addition).is_some());
        assert!(function.operation(right_addition).is_some());
    }

    #[test]
    fn preserves_repeated_expressions_that_may_be_observable() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let (first, repeated) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);

            let unknown = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::LoadGlobal(LoadGlobalOp::new("value")),
                [],
                UnwindTarget::Propagate,
            );
            let unknown = builder.operation_results(unknown)[0];
            let one = append_number(&mut builder, 1.0);

            let (first, _) = append_addition(&mut builder, unknown, one);
            let (repeated, repeated_result) = append_addition(&mut builder, unknown, one);

            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [repeated_result],
                UnwindTarget::Propagate,
            );

            (first, repeated)
        };

        assert_eq!(
            eliminate_common_subexpressions(module.function_mut(function).unwrap()),
            0
        );

        let function = module.function(function).unwrap();

        assert!(function.operation(first).is_some());
        assert!(function.operation(repeated).is_some());
    }

    fn append_addition(
        builder: &mut evrel_js_ir::FunctionBuilder<'_>,
        left: ValueId,
        right: ValueId,
    ) -> (OperationId, ValueId) {
        let operation = builder.append_operation(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::Binary(BinaryOp::new(BinaryOperator::Add)),
            [left, right],
            UnwindTarget::Propagate,
        );

        (operation, builder.operation_results(operation)[0])
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
