//! Canonical simplification of ordinary regional control flow.

use evrel_js_ir::{
    BlockId, BlockParameterSource, FunctionEditor, JsFunctionIr, OperationId, OperationKind,
    ValueId,
};
use rustc_hash::FxHashMap;

use crate::js::analysis::{FunctionValueAnalysis, RegionControlFlowGraph};

/// Folds constant conditions, removes forwarding blocks, and merges linear
/// block chains.
///
/// Constant-condition planning uses one immutable value-analysis snapshot.
/// Local structural rewrites recompute regional control flow after every
/// mutation.
///
/// Returns the number of applied control-flow rewrites.
pub fn simplify_control_flow(function: &mut JsFunctionIr) -> usize {
    let mut rewritten = fold_constant_ifs(function);

    while let Some(rewrite) = find_rewrite(function) {
        apply_rewrite(function, rewrite);
        rewritten += 1;
    }

    rewritten
}

struct FoldConstantIf {
    terminator: OperationId,
    successor_index: usize,
}

fn fold_constant_ifs(function: &mut JsFunctionIr) -> usize {
    let rewrites = {
        let analysis = FunctionValueAnalysis::analyze(function);

        function
            .operations()
            .filter_map(|(operation, data)| {
                if !matches!(data.kind(), OperationKind::If(_)) {
                    return None;
                }

                let successor_index = analysis.unique_executable_successor(function, operation)?;

                Some(FoldConstantIf {
                    terminator: operation,
                    successor_index,
                })
            })
            .collect::<Vec<_>>()
    };

    let rewritten = rewrites.len();
    let mut editor = FunctionEditor::new(function);

    for rewrite in rewrites {
        editor.replace_if_with_jump(rewrite.terminator, rewrite.successor_index);
    }

    rewritten
}

enum Rewrite {
    ThreadForwardingBlock(ThreadForwardingBlock),
    MergeLinearBlock(MergeLinearBlock),
}

fn find_rewrite(function: &JsFunctionIr) -> Option<Rewrite> {
    for (region, _) in function.regions() {
        let control_flow = RegionControlFlowGraph::compute(function, region);

        for &block in control_flow.blocks() {
            if let Some(rewrite) = plan_forwarding_block(function, &control_flow, block) {
                return Some(Rewrite::ThreadForwardingBlock(rewrite));
            }

            if let Some(rewrite) = plan_linear_block(function, &control_flow, block) {
                return Some(Rewrite::MergeLinearBlock(rewrite));
            }
        }
    }

    None
}

fn apply_rewrite(function: &mut JsFunctionIr, rewrite: Rewrite) {
    let mut editor = FunctionEditor::new(function);

    match rewrite {
        Rewrite::ThreadForwardingBlock(rewrite) => {
            for incoming in rewrite.incoming {
                editor.replace_successor(
                    incoming.terminator,
                    incoming.successor_index,
                    rewrite.target,
                    incoming.arguments,
                );
            }

            editor.remove_blocks([rewrite.block]);
        }

        Rewrite::MergeLinearBlock(rewrite) => {
            for (parameter, argument) in rewrite.substitutions {
                editor.replace_all_uses(parameter, argument);
            }

            editor.merge_block_into_predecessor(rewrite.predecessor, rewrite.block);
        }
    }
}

struct ThreadForwardingBlock {
    block: BlockId,
    target: BlockId,
    incoming: Vec<ThreadedEdge>,
}

struct ThreadedEdge {
    terminator: OperationId,
    successor_index: usize,
    arguments: Vec<ValueId>,
}

fn plan_forwarding_block(
    function: &JsFunctionIr,
    control_flow: &RegionControlFlowGraph,
    block: BlockId,
) -> Option<ThreadForwardingBlock> {
    if block_has_boundary_role(function, block) {
        return None;
    }

    let block_data = function.block(block)?;

    if !block_data.operations().is_empty()
        || block_data
            .parameters()
            .iter()
            .any(|parameter| parameter.source() != BlockParameterSource::Forwarded)
    {
        return None;
    }

    let terminator = block_data.terminator()?;
    let operation = function.operation(terminator)?;

    let OperationKind::Jump(jump) = operation.kind() else {
        return None;
    };

    let target = jump.target().block();

    if target == block {
        return None;
    }

    let incoming_edges = control_flow.predecessor_edges(block)?;

    if incoming_edges.is_empty() {
        return None;
    }

    if block_data.parameters().iter().any(|parameter| {
        function.value(parameter.value()).is_none_or(|value| {
            value
                .uses()
                .iter()
                .any(|use_site| use_site.operation() != terminator)
        })
    }) {
        return None;
    }

    let parameter_indices = block_data
        .parameters()
        .iter()
        .enumerate()
        .map(|(index, parameter)| (parameter.value(), index))
        .collect::<FxHashMap<_, _>>();

    let outgoing = operation.successors().into_iter().next()?;
    let outgoing_arguments = outgoing.arguments(operation.operands());

    // An outgoing value defined outside this block would need a dominance
    // proof for every incoming edge. Pure parameter composition needs none.
    if outgoing_arguments
        .iter()
        .any(|argument| !parameter_indices.contains_key(argument))
    {
        return None;
    }

    let mut incoming = Vec::with_capacity(incoming_edges.len());

    for &edge_id in incoming_edges {
        let edge = control_flow.edge(edge_id)?;
        let predecessor = function.operation(edge.terminator())?;
        let successor = *predecessor.successors().get(edge.successor_index())?;

        if successor.produced_argument_count() != 0 {
            return None;
        }

        let incoming_arguments = successor.arguments(predecessor.operands());

        if incoming_arguments.len() != block_data.parameters().len() {
            return None;
        }

        let arguments = outgoing_arguments
            .iter()
            .map(|argument| incoming_arguments[parameter_indices[argument]])
            .collect();

        incoming.push(ThreadedEdge {
            terminator: edge.terminator(),
            successor_index: edge.successor_index(),
            arguments,
        });
    }

    Some(ThreadForwardingBlock {
        block,
        target,
        incoming,
    })
}

struct MergeLinearBlock {
    predecessor: BlockId,
    block: BlockId,
    substitutions: Vec<(ValueId, ValueId)>,
}

fn plan_linear_block(
    function: &JsFunctionIr,
    control_flow: &RegionControlFlowGraph,
    block: BlockId,
) -> Option<MergeLinearBlock> {
    if block_has_boundary_role(function, block) || function.block(block)?.terminator().is_none() {
        return None;
    }

    let [incoming_edge] = control_flow.predecessor_edges(block)? else {
        return None;
    };

    let incoming = control_flow.edge(*incoming_edge)?;
    let predecessor = incoming.source();

    if predecessor == block || control_flow.successor_edges(predecessor)?.len() != 1 {
        return None;
    }

    let predecessor_terminator = function.operation(incoming.terminator())?;

    if !matches!(predecessor_terminator.kind(), OperationKind::Jump(_)) {
        return None;
    }

    let successor = *predecessor_terminator
        .successors()
        .get(incoming.successor_index())?;
    let arguments = successor.arguments(predecessor_terminator.operands());
    let parameters = function.block(block)?.parameters();

    if successor.produced_argument_count() != 0
        || parameters.len() != arguments.len()
        || parameters
            .iter()
            .any(|parameter| parameter.source() != BlockParameterSource::Forwarded)
    {
        return None;
    }

    let substitutions = parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (parameter.value(), *argument))
        .collect::<Vec<_>>();

    if substitutions
        .iter()
        .any(|(parameter, argument)| parameter == argument)
    {
        return None;
    }

    Some(MergeLinearBlock {
        predecessor,
        block,
        substitutions,
    })
}

fn block_has_boundary_role(function: &JsFunctionIr, block: BlockId) -> bool {
    function
        .block_region(block)
        .and_then(|region| function.region(region))
        .is_none_or(|region| region.entry_block() == block)
        || function
            .exception_handlers()
            .any(|(_, handler)| handler.entry_block() == block)
        || function.labeled_statements().any(|(_, statement)| {
            statement.body_block() == block || statement.completion_block() == block
        })
        || function
            .loop_operations()
            .any(|(_, operation)| operation.blocks().any(|candidate| candidate == block))
        || function.operations().any(|(_, operation)| {
            operation
                .structural_blocks()
                .into_iter()
                .any(|candidate| candidate == block)
        })
}

#[cfg(test)]
mod tests {
    use evrel_js_ir::{
        BlockParameterSource, BlockTarget, ConstantOp, ConstantValue, IfOp, JsModuleIr, JumpOp,
        LoadThisOp, ModuleBuilder, OperationKind, ReturnOp,
    };

    use super::{fold_constant_ifs, simplify_control_flow};

    #[test]
    fn folds_truthy_and_falsy_conditions_and_repairs_uses() {
        for condition in [true, false] {
            let mut module = JsModuleIr::new();
            let function = module.entry_function();

            let (terminator, condition_value, then_block, else_block, then_value, else_value) = {
                let mut module_builder = ModuleBuilder::new(&mut module);
                let mut builder = module_builder.function_builder(function);
                let then_block = builder.create_block();
                let else_block = builder.create_block();
                let completion = builder.create_block();
                let then_parameter = builder.append_block_parameter(
                    then_block,
                    BlockParameterSource::Forwarded,
                    evrel_js_ir::ValueType::JsValue,
                );
                let else_parameter = builder.append_block_parameter(
                    else_block,
                    BlockParameterSource::Forwarded,
                    evrel_js_ir::ValueType::JsValue,
                );
                let condition_value = append_boolean(&mut builder, condition);
                let then_value = append_number(&mut builder, 1.0);
                let else_value = append_number(&mut builder, 2.0);
                let terminator = builder.terminate(
                    evrel_js_ir::LocationId::UNKNOWN,
                    OperationKind::If(IfOp::new(
                        BlockTarget::new(then_block, 1),
                        BlockTarget::new(else_block, 1),
                        completion,
                    )),
                    [condition_value, then_value, else_value],
                );

                builder.switch_to_block(then_block);
                builder.terminate(
                    evrel_js_ir::LocationId::UNKNOWN,
                    OperationKind::Return(ReturnOp::new()),
                    [then_parameter],
                );

                builder.switch_to_block(else_block);
                builder.terminate(
                    evrel_js_ir::LocationId::UNKNOWN,
                    OperationKind::Return(ReturnOp::new()),
                    [else_parameter],
                );

                builder.switch_to_block(completion);
                let completion_value = append_number(&mut builder, 0.0);
                builder.terminate(
                    evrel_js_ir::LocationId::UNKNOWN,
                    OperationKind::Return(ReturnOp::new()),
                    [completion_value],
                );

                (
                    terminator,
                    condition_value,
                    then_block,
                    else_block,
                    then_value,
                    else_value,
                )
            };

            assert_eq!(fold_constant_ifs(module.function_mut(function).unwrap()), 1);

            let function = module.function(function).unwrap();
            let operation = function.operation(terminator).unwrap();
            let selected_block = if condition { then_block } else { else_block };
            let selected_value = if condition { then_value } else { else_value };
            let rejected_value = if condition { else_value } else { then_value };

            assert!(matches!(operation.kind(), OperationKind::Jump(_)));
            assert_eq!(
                successor(function, function.entry_block()),
                (selected_block, vec![selected_value])
            );
            assert!(function.value(condition_value).unwrap().uses().is_empty());
            assert!(function.value(rejected_value).unwrap().uses().is_empty());
            assert_eq!(function.value(selected_value).unwrap().uses().len(), 1);
        }
    }

    #[test]
    fn threads_a_forwarding_block_and_composes_its_arguments() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let (left, right, forwarding, target, left_value, right_value, target_parameter) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let left = builder.create_block();
            let right = builder.create_block();
            let forwarding = builder.create_block();
            let target = builder.create_block();
            let forwarding_parameter = builder.append_block_parameter(
                forwarding,
                BlockParameterSource::Forwarded,
                evrel_js_ir::ValueType::JsValue,
            );
            let target_parameter = builder.append_block_parameter(
                target,
                BlockParameterSource::Forwarded,
                evrel_js_ir::ValueType::JsValue,
            );

            let condition = append_unknown_condition(&mut builder);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::If(IfOp::new(
                    BlockTarget::new(left, 0),
                    BlockTarget::new(right, 0),
                    target,
                )),
                [condition],
            );

            builder.switch_to_block(left);
            let left_value = append_number(&mut builder, 1.0);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(forwarding, 1))),
                [left_value],
            );

            builder.switch_to_block(right);
            let right_value = append_number(&mut builder, 2.0);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(forwarding, 1))),
                [right_value],
            );

            builder.switch_to_block(forwarding);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(target, 1))),
                [forwarding_parameter],
            );

            builder.switch_to_block(target);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [target_parameter],
            );

            (
                left,
                right,
                forwarding,
                target,
                left_value,
                right_value,
                target_parameter,
            )
        };

        assert_eq!(
            simplify_control_flow(module.function_mut(function).unwrap()),
            1
        );
        assert_eq!(
            simplify_control_flow(module.function_mut(function).unwrap()),
            0
        );

        let function = module.function(function).unwrap();
        assert!(function.block(forwarding).is_none());
        assert_eq!(successor(function, left), (target, vec![left_value]),);
        assert_eq!(successor(function, right), (target, vec![right_value]),);
        assert!(function.value(target_parameter).is_some());
    }

    #[test]
    fn threads_a_forwarding_block_that_discards_its_argument() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let (left, right, forwarding, target, left_argument, right_argument) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let left = builder.create_block();
            let right = builder.create_block();
            let forwarding = builder.create_block();
            let target = builder.create_block();
            builder.append_block_parameter(
                forwarding,
                BlockParameterSource::Forwarded,
                evrel_js_ir::ValueType::JsValue,
            );

            let condition = append_unknown_condition(&mut builder);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::If(IfOp::new(
                    BlockTarget::new(left, 0),
                    BlockTarget::new(right, 0),
                    target,
                )),
                [condition],
            );

            builder.switch_to_block(left);
            let left_argument = append_number(&mut builder, 1.0);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(forwarding, 1))),
                [left_argument],
            );

            builder.switch_to_block(right);
            let right_argument = append_number(&mut builder, 2.0);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(forwarding, 1))),
                [right_argument],
            );

            builder.switch_to_block(forwarding);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(target, 0))),
                [],
            );

            builder.switch_to_block(target);
            let returned = append_number(&mut builder, 3.0);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [returned],
            );

            (
                left,
                right,
                forwarding,
                target,
                left_argument,
                right_argument,
            )
        };

        assert_eq!(
            simplify_control_flow(module.function_mut(function).unwrap()),
            1
        );

        let function = module.function(function).unwrap();
        assert!(function.block(forwarding).is_none());
        assert_eq!(successor(function, left), (target, Vec::new()));
        assert_eq!(successor(function, right), (target, Vec::new()));
        assert!(function.value(left_argument).unwrap().uses().is_empty());
        assert!(function.value(right_argument).unwrap().uses().is_empty());
    }

    #[test]
    fn merges_a_linear_block_and_substitutes_its_parameters() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let (middle, argument, returned) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let middle = builder.create_block();
            let parameter = builder.append_block_parameter(
                middle,
                BlockParameterSource::Forwarded,
                evrel_js_ir::ValueType::JsValue,
            );
            let argument = append_number(&mut builder, 42.0);

            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(middle, 1))),
                [argument],
            );

            builder.switch_to_block(middle);
            let returned = builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [parameter],
            );

            (middle, argument, returned)
        };

        assert_eq!(
            simplify_control_flow(module.function_mut(function).unwrap()),
            1
        );

        let function = module.function(function).unwrap();
        assert!(function.block(middle).is_none());
        assert_eq!(
            function.operation(returned).unwrap().block(),
            function.entry_block(),
        );
        assert_eq!(function.operation(returned).unwrap().operands(), [argument]);
    }

    #[test]
    fn preserves_a_multi_predecessor_parameter_used_in_the_forwarding_block() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let forwarding = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let left = builder.create_block();
            let right = builder.create_block();
            let forwarding = builder.create_block();
            let target = builder.create_block();
            let parameter = builder.append_block_parameter(
                forwarding,
                BlockParameterSource::Forwarded,
                evrel_js_ir::ValueType::JsValue,
            );

            let condition = append_unknown_condition(&mut builder);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::If(IfOp::new(
                    BlockTarget::new(left, 0),
                    BlockTarget::new(right, 0),
                    target,
                )),
                [condition],
            );

            for (block, value) in [(left, 1.0), (right, 2.0)] {
                builder.switch_to_block(block);
                let value = append_number(&mut builder, value);
                builder.terminate(
                    evrel_js_ir::LocationId::UNKNOWN,
                    OperationKind::Jump(JumpOp::new(BlockTarget::new(forwarding, 1))),
                    [value],
                );
            }

            builder.switch_to_block(forwarding);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(target, 0))),
                [],
            );

            builder.switch_to_block(target);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [parameter],
            );

            forwarding
        };

        assert_eq!(
            simplify_control_flow(module.function_mut(function).unwrap()),
            0
        );
        assert!(
            module
                .function(function)
                .unwrap()
                .block(forwarding)
                .is_some()
        );
    }

    fn append_boolean(
        builder: &mut evrel_js_ir::FunctionBuilder<'_>,
        value: bool,
    ) -> evrel_js_ir::ValueId {
        let operation = builder.append_operation(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::Constant(ConstantOp::new(ConstantValue::Boolean(value))),
            [],
        );

        builder.operation_results(operation)[0]
    }

    fn append_unknown_condition(
        builder: &mut evrel_js_ir::FunctionBuilder<'_>,
    ) -> evrel_js_ir::ValueId {
        let operation = builder.append_operation(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::LoadThis(LoadThisOp::new()),
            [],
        );

        builder.operation_results(operation)[0]
    }

    fn append_number(
        builder: &mut evrel_js_ir::FunctionBuilder<'_>,
        value: f64,
    ) -> evrel_js_ir::ValueId {
        let operation = builder.append_operation(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::Constant(ConstantOp::new(ConstantValue::Number(value))),
            [],
        );

        builder.operation_results(operation)[0]
    }

    fn successor(
        function: &evrel_js_ir::JsFunctionIr,
        block: evrel_js_ir::BlockId,
    ) -> (evrel_js_ir::BlockId, Vec<evrel_js_ir::ValueId>) {
        let terminator = function.block(block).unwrap().terminator().unwrap();
        let operation = function.operation(terminator).unwrap();
        let successor = operation.successors()[0];

        (
            successor.target().block(),
            successor.arguments(operation.operands()).to_vec(),
        )
    }
}
