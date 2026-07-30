//! Elimination of trivial forwarded SSA block parameters.

use evrel_ir::{
    BlockId, BlockParameterSource, FunctionEditor, FunctionIr, ValueDefinition, ValueId,
};

use crate::analysis::{RegionControlFlowError, RegionControlFlowGraph};
use crate::work_queue::WorkQueue;

#[derive(Debug, Clone, Copy)]
struct Simplification {
    block: BlockId,
    forwarded_index: usize,
    parameter: ValueId,
    replacement: Option<ValueId>,
}

/// Removes unused forwarded block parameters and parameters whose effective
/// incoming values all agree.
///
/// Returns zero when the function's ordinary regional control flow cannot be
/// modeled soundly.
pub fn simplify_block_parameters(function: &mut FunctionIr) -> usize {
    let graphs = function
        .regions()
        .map(|(region, _)| RegionControlFlowGraph::compute(function, region))
        .collect::<Result<Vec<_>, RegionControlFlowError>>();

    let Ok(graphs) = graphs else {
        return 0;
    };

    let mut work = WorkQueue::new();

    for graph in &graphs {
        for &block in graph.blocks() {
            work.push(block);
        }
    }

    let mut editor = FunctionEditor::new(function);
    let mut removed = 0;

    while let Some(block) = work.pop() {
        let Some(region) = editor.ir().block_region(block) else {
            continue;
        };
        let graph = graphs
            .iter()
            .find(|graph| graph.region() == region)
            .expect("every function region must have a control-flow graph");

        let Some(simplification) = find_simplification(editor.ir(), graph, block) else {
            continue;
        };

        let affected = match simplification.replacement {
            Some(replacement) => {
                let affected = successor_argument_targets(editor.ir(), simplification.parameter);

                editor.replace_all_uses(simplification.parameter, replacement);
                affected
            }

            None => incoming_argument_parameter_blocks(
                editor.ir(),
                graph,
                simplification.block,
                simplification.forwarded_index,
            ),
        };

        let removed_parameter = editor
            .remove_forwarded_block_parameter(simplification.block, simplification.forwarded_index);

        debug_assert_eq!(removed_parameter, simplification.parameter);

        removed += 1;
        work.push(block);

        for affected_block in affected {
            work.push(affected_block);
        }
    }

    removed
}

fn find_simplification(
    function: &FunctionIr,
    graph: &RegionControlFlowGraph,
    block: BlockId,
) -> Option<Simplification> {
    let predecessors = graph
        .predecessor_edges(block)
        .expect("control-flow block must have predecessor storage");

    if predecessors.is_empty() {
        return None;
    }

    let block_data = function
        .block(block)
        .expect("control-flow block must remain live");

    for (forwarded_index, parameter) in block_data
        .parameters()
        .iter()
        .filter(|parameter| parameter.source() == BlockParameterSource::Forwarded)
        .enumerate()
    {
        let parameter = parameter.value();
        let value = function
            .value(parameter)
            .expect("block parameter must reference a live value");

        if value.uses().is_empty() {
            return Some(Simplification {
                block,
                forwarded_index,
                parameter,
                replacement: None,
            });
        }

        let mut replacement = None;
        let mut all_non_self_inputs_match = true;

        for &edge in predecessors {
            let edge = graph
                .edge(edge)
                .expect("predecessor edge must remain in the graph");
            let operation = function
                .operation(edge.terminator())
                .expect("predecessor terminator must remain live");
            let successor = operation.successors()[edge.successor_index()];
            let argument = successor.arguments(operation.operands())[forwarded_index];

            // A loop backedge forwarding the parameter itself introduces no
            // new value and therefore does not affect the replacement.
            if argument == parameter {
                continue;
            }

            match replacement {
                // The first effective input becomes the candidate replacement.
                None => replacement = Some(argument),

                // Another edge forwards the same effective input.
                Some(current) if current == argument => {}

                // This parameter has at least two different effective inputs.
                Some(_) => {
                    all_non_self_inputs_match = false;
                    break;
                }
            }
        }

        if all_non_self_inputs_match && let Some(replacement) = replacement {
            return Some(Simplification {
                block,
                forwarded_index,
                parameter,
                replacement: Some(replacement),
            });
        }
    }

    None
}

fn successor_argument_targets(function: &FunctionIr, value: ValueId) -> Vec<BlockId> {
    let value = function
        .value(value)
        .expect("simplified parameter must remain live");
    let mut targets = Vec::new();

    for use_site in value.uses() {
        let operation = function
            .operation(use_site.operation())
            .expect("value use must reference a live operation");
        let operand_index = use_site.operand_index() as usize;

        for successor in operation.successors() {
            if successor.argument_operand_range().contains(&operand_index) {
                targets.push(successor.target().block());
            }
        }
    }

    targets
}

fn incoming_argument_parameter_blocks(
    function: &FunctionIr,
    graph: &RegionControlFlowGraph,
    block: BlockId,
    forwarded_index: usize,
) -> Vec<BlockId> {
    let mut blocks = Vec::new();

    for &edge in graph
        .predecessor_edges(block)
        .expect("control-flow block must have predecessor storage")
    {
        let edge = graph
            .edge(edge)
            .expect("predecessor edge must remain in the graph");
        let operation = function
            .operation(edge.terminator())
            .expect("predecessor terminator must remain live");
        let successor = operation.successors()[edge.successor_index()];
        let argument = successor.arguments(operation.operands())[forwarded_index];

        if let Some(ValueDefinition::BlockParameter { block, .. }) =
            function.value(argument).map(|value| value.definition())
        {
            blocks.push(*block);
        }
    }

    blocks
}

#[cfg(test)]
mod tests {
    use evrel_ir::{
        BinaryOp, BinaryOperator, BlockParameterSource, BlockTarget, ConstantOp, ConstantValue,
        FunctionBuilder, IfOp, JumpOp, ModuleBuilder, ModuleIr, OperationKind, ReturnOp,
        UnwindTarget, ValueDefinition, ValueId,
    };

    use super::simplify_block_parameters;

    #[test]
    fn replaces_a_parameter_when_every_input_matches() {
        let mut module = ModuleIr::new();
        let function = module.entry_function();

        let (join, parameter, condition, value, branch, returned) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let join = builder.create_block();
            let parameter = builder.append_block_parameter(join, BlockParameterSource::Forwarded);
            let condition = append_boolean(&mut builder, true);
            let value = append_number(&mut builder, 1.0);
            let branch = builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::If(IfOp::new(
                    BlockTarget::new(join, 1),
                    BlockTarget::new(join, 1),
                    join,
                )),
                [condition, value, value],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(join);
            let returned = builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [parameter],
                UnwindTarget::Propagate,
            );

            (join, parameter, condition, value, branch, returned)
        };

        assert_eq!(
            simplify_block_parameters(module.function_mut(function).unwrap()),
            1
        );
        assert_eq!(
            simplify_block_parameters(module.function_mut(function).unwrap()),
            0
        );

        let function = module.function(function).unwrap();
        let branch = function.operation(branch).unwrap();

        assert!(function.block(join).unwrap().parameters().is_empty());
        assert!(function.value(parameter).is_none());
        assert_eq!(branch.operands(), [condition]);
        assert!(
            branch
                .successors()
                .iter()
                .all(|successor| successor.target().argument_count() == 0),
        );
        assert_eq!(function.operation(returned).unwrap().operands(), [value]);
    }

    #[test]
    fn continues_after_a_parameter_with_distinct_inputs() {
        let mut module = ModuleIr::new();
        let function = module.entry_function();

        let (join, distinct, matching, left, right, matching_value, branch, comparison) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let join = builder.create_block();
            let distinct = builder.append_block_parameter(join, BlockParameterSource::Forwarded);
            let matching = builder.append_block_parameter(join, BlockParameterSource::Forwarded);
            let condition = append_boolean(&mut builder, true);
            let left = append_number(&mut builder, 1.0);
            let right = append_number(&mut builder, 2.0);
            let matching_value = append_number(&mut builder, 3.0);
            let branch = builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::If(IfOp::new(
                    BlockTarget::new(join, 2),
                    BlockTarget::new(join, 2),
                    join,
                )),
                [condition, left, matching_value, right, matching_value],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(join);
            let comparison = builder.append_operation(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Binary(BinaryOp::new(BinaryOperator::StrictEqual)),
                [distinct, matching],
                UnwindTarget::Propagate,
            );
            let comparison_result = builder.operation_results(comparison)[0];
            builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [comparison_result],
                UnwindTarget::Propagate,
            );

            (
                join,
                distinct,
                matching,
                left,
                right,
                matching_value,
                branch,
                comparison,
            )
        };

        assert_eq!(
            simplify_block_parameters(module.function_mut(function).unwrap()),
            1
        );

        let function = module.function(function).unwrap();
        let block = function.block(join).unwrap();
        let branch = function.operation(branch).unwrap();

        assert_eq!(block.parameters().len(), 1);
        assert_eq!(block.parameters()[0].value(), distinct);
        assert!(function.value(matching).is_none());
        assert!(matches!(
            function.value(distinct).unwrap().definition(),
            ValueDefinition::BlockParameter {
                block: parameter_block,
                parameter_index: 0,
            } if *parameter_block == join
        ));
        let successors = branch.successors();
        assert_eq!(successors[0].arguments(branch.operands()), [left]);
        assert_eq!(successors[1].arguments(branch.operands()), [right]);
        assert_eq!(
            function.operation(comparison).unwrap().operands(),
            [distinct, matching_value],
        );
    }

    #[test]
    fn removes_an_unused_parameter_even_when_inputs_differ() {
        let mut module = ModuleIr::new();
        let function = module.entry_function();

        let (join, parameter, branch) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let join = builder.create_block();
            let parameter = builder.append_block_parameter(join, BlockParameterSource::Forwarded);
            let condition = append_boolean(&mut builder, true);
            let left = append_number(&mut builder, 1.0);
            let right = append_number(&mut builder, 2.0);
            let branch = builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::If(IfOp::new(
                    BlockTarget::new(join, 1),
                    BlockTarget::new(join, 1),
                    join,
                )),
                [condition, left, right],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(join);
            let undefined = append_undefined(&mut builder);
            builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [undefined],
                UnwindTarget::Propagate,
            );

            (join, parameter, branch)
        };

        assert_eq!(
            simplify_block_parameters(module.function_mut(function).unwrap()),
            1
        );

        let function = module.function(function).unwrap();
        let branch = function.operation(branch).unwrap();

        assert!(function.block(join).unwrap().parameters().is_empty());
        assert!(function.value(parameter).is_none());
        assert_eq!(branch.operands().len(), 1);
    }

    #[test]
    fn ignores_a_self_referential_loop_backedge() {
        let mut module = ModuleIr::new();
        let function = module.entry_function();

        let (header, parameter, initial, entry_jump, backedge, returned) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let header = builder.create_block();
            let body = builder.create_block();
            let exit = builder.create_block();
            let parameter = builder.append_block_parameter(header, BlockParameterSource::Forwarded);
            let initial = append_number(&mut builder, 1.0);
            let entry_jump = builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(header, 1))),
                [initial],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(header);
            let condition = append_boolean(&mut builder, true);
            builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::If(IfOp::new(
                    BlockTarget::new(body, 0),
                    BlockTarget::new(exit, 0),
                    exit,
                )),
                [condition],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(body);
            let backedge = builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(header, 1))),
                [parameter],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(exit);
            let returned = builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [parameter],
                UnwindTarget::Propagate,
            );

            (header, parameter, initial, entry_jump, backedge, returned)
        };

        assert_eq!(
            simplify_block_parameters(module.function_mut(function).unwrap()),
            1
        );

        let function = module.function(function).unwrap();

        assert!(function.block(header).unwrap().parameters().is_empty());
        assert!(function.value(parameter).is_none());
        assert!(
            function
                .operation(entry_jump)
                .unwrap()
                .operands()
                .is_empty()
        );
        assert!(function.operation(backedge).unwrap().operands().is_empty());
        assert_eq!(function.operation(returned).unwrap().operands(), [initial]);
    }

    #[test]
    fn removes_transitively_unused_parameters_to_a_fixed_point() {
        let mut module = ModuleIr::new();
        let function = module.entry_function();

        let (forwarding, forwarded, completion, result, entry_jump, forwarding_jump) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let forwarding = builder.create_block();
            let forwarded =
                builder.append_block_parameter(forwarding, BlockParameterSource::Forwarded);
            let completion = builder.create_block();
            let result =
                builder.append_block_parameter(completion, BlockParameterSource::Forwarded);
            let initial = append_number(&mut builder, 1.0);
            let entry_jump = builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(forwarding, 1))),
                [initial],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(forwarding);
            let forwarding_jump = builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(completion, 1))),
                [forwarded],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(completion);
            let undefined = append_undefined(&mut builder);
            builder.terminate(
                evrel_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [undefined],
                UnwindTarget::Propagate,
            );

            (
                forwarding,
                forwarded,
                completion,
                result,
                entry_jump,
                forwarding_jump,
            )
        };

        assert_eq!(
            simplify_block_parameters(module.function_mut(function).unwrap()),
            2
        );
        assert_eq!(
            simplify_block_parameters(module.function_mut(function).unwrap()),
            0
        );

        let function = module.function(function).unwrap();

        assert!(function.block(forwarding).unwrap().parameters().is_empty());
        assert!(function.block(completion).unwrap().parameters().is_empty());
        assert!(function.value(forwarded).is_none());
        assert!(function.value(result).is_none());
        assert!(
            function
                .operation(entry_jump)
                .unwrap()
                .operands()
                .is_empty()
        );
        assert!(
            function
                .operation(forwarding_jump)
                .unwrap()
                .operands()
                .is_empty(),
        );
    }

    fn append_boolean(builder: &mut FunctionBuilder<'_>, value: bool) -> ValueId {
        append_constant(builder, ConstantValue::Boolean(value))
    }

    fn append_number(builder: &mut FunctionBuilder<'_>, value: f64) -> ValueId {
        append_constant(builder, ConstantValue::Number(value))
    }

    fn append_undefined(builder: &mut FunctionBuilder<'_>) -> ValueId {
        append_constant(builder, ConstantValue::Undefined)
    }

    fn append_constant(builder: &mut FunctionBuilder<'_>, value: ConstantValue) -> ValueId {
        let operation = builder.append_operation(
            evrel_ir::LocationId::UNKNOWN,
            OperationKind::Constant(ConstantOp::new(value)),
            [],
            UnwindTarget::Propagate,
        );

        builder.operation_results(operation)[0]
    }
}
