//! Canonical simplification of ordinary regional control flow.

use evrel_ir::{
    BlockId, BlockParameterSource, FunctionEditor, FunctionIr, ModuleIr, OperationId,
    OperationKind, ValueId,
};
use rustc_hash::FxHashMap;

use crate::analysis::RegionControlFlowGraph;

/// Removes forwarding blocks and merges linear block chains.
///
/// Each rewrite removes exactly one block. Control flow is recomputed after
/// every mutation, keeping planning immutable and guaranteeing termination
/// without an iteration limit.
///
/// Returns the number of removed blocks.
pub fn simplify_control_flow(module: &mut ModuleIr) -> usize {
    module
        .functions_mut()
        .map(|(_, function)| simplify_function(function))
        .sum()
}

fn simplify_function(function: &mut FunctionIr) -> usize {
    let mut removed = 0;

    while let Some(rewrite) = find_rewrite(function) {
        apply_rewrite(function, rewrite);
        removed += 1;
    }

    removed
}

enum Rewrite {
    ThreadForwardingBlock(ThreadForwardingBlock),
    MergeLinearBlock(MergeLinearBlock),
}

fn find_rewrite(function: &FunctionIr) -> Option<Rewrite> {
    for (region, _) in function.regions() {
        let Ok(control_flow) = RegionControlFlowGraph::compute(function, region) else {
            continue;
        };

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

fn apply_rewrite(function: &mut FunctionIr, rewrite: Rewrite) {
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
    function: &FunctionIr,
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
    function: &FunctionIr,
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

fn block_has_boundary_role(function: &FunctionIr, block: BlockId) -> bool {
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
    use evrel_ir::{
        BlockParameterSource, BlockTarget, ConstantOp, ConstantValue, IfOp, JumpOp, ModuleBuilder,
        ModuleIr, OperationKind, ReturnOp, UnwindTarget,
    };

    use super::simplify_control_flow;

    #[test]
    fn threads_a_forwarding_block_and_composes_its_arguments() {
        let mut module = ModuleIr::new();
        let function = module.entry_function();

        let (left, right, forwarding, target, left_value, right_value, target_parameter) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let left = builder.create_block();
            let right = builder.create_block();
            let forwarding = builder.create_block();
            let target = builder.create_block();
            let forwarding_parameter =
                builder.append_block_parameter(forwarding, BlockParameterSource::Forwarded);
            let target_parameter =
                builder.append_block_parameter(target, BlockParameterSource::Forwarded);

            let condition = append_boolean(&mut builder, true);
            builder.terminate(
                OperationKind::If(IfOp::new(
                    BlockTarget::new(left, 0),
                    BlockTarget::new(right, 0),
                    target,
                )),
                [condition],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(left);
            let left_value = append_number(&mut builder, 1.0);
            builder.terminate(
                OperationKind::Jump(JumpOp::new(BlockTarget::new(forwarding, 1))),
                [left_value],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(right);
            let right_value = append_number(&mut builder, 2.0);
            builder.terminate(
                OperationKind::Jump(JumpOp::new(BlockTarget::new(forwarding, 1))),
                [right_value],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(forwarding);
            builder.terminate(
                OperationKind::Jump(JumpOp::new(BlockTarget::new(target, 1))),
                [forwarding_parameter],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(target);
            builder.terminate(
                OperationKind::Return(ReturnOp::new()),
                [target_parameter],
                UnwindTarget::Propagate,
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

        assert_eq!(simplify_control_flow(&mut module), 1);
        assert_eq!(simplify_control_flow(&mut module), 0);

        let function = module.function(function).unwrap();
        assert!(function.block(forwarding).is_none());
        assert_eq!(successor(function, left), (target, vec![left_value]),);
        assert_eq!(successor(function, right), (target, vec![right_value]),);
        assert!(function.value(target_parameter).is_some());
    }

    #[test]
    fn merges_a_linear_block_and_substitutes_its_parameters() {
        let mut module = ModuleIr::new();
        let function = module.entry_function();

        let (middle, argument, returned) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let middle = builder.create_block();
            let parameter = builder.append_block_parameter(middle, BlockParameterSource::Forwarded);
            let argument = append_number(&mut builder, 42.0);

            builder.terminate(
                OperationKind::Jump(JumpOp::new(BlockTarget::new(middle, 1))),
                [argument],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(middle);
            let returned = builder.terminate(
                OperationKind::Return(ReturnOp::new()),
                [parameter],
                UnwindTarget::Propagate,
            );

            (middle, argument, returned)
        };

        assert_eq!(simplify_control_flow(&mut module), 1);

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
        let mut module = ModuleIr::new();
        let function = module.entry_function();

        let forwarding = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let left = builder.create_block();
            let right = builder.create_block();
            let forwarding = builder.create_block();
            let target = builder.create_block();
            let parameter =
                builder.append_block_parameter(forwarding, BlockParameterSource::Forwarded);

            let condition = append_boolean(&mut builder, true);
            builder.terminate(
                OperationKind::If(IfOp::new(
                    BlockTarget::new(left, 0),
                    BlockTarget::new(right, 0),
                    target,
                )),
                [condition],
                UnwindTarget::Propagate,
            );

            for (block, value) in [(left, 1.0), (right, 2.0)] {
                builder.switch_to_block(block);
                let value = append_number(&mut builder, value);
                builder.terminate(
                    OperationKind::Jump(JumpOp::new(BlockTarget::new(forwarding, 1))),
                    [value],
                    UnwindTarget::Propagate,
                );
            }

            builder.switch_to_block(forwarding);
            builder.terminate(
                OperationKind::Jump(JumpOp::new(BlockTarget::new(target, 0))),
                [],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(target);
            builder.terminate(
                OperationKind::Return(ReturnOp::new()),
                [parameter],
                UnwindTarget::Propagate,
            );

            forwarding
        };

        assert_eq!(simplify_control_flow(&mut module), 0);
        assert!(
            module
                .function(function)
                .unwrap()
                .block(forwarding)
                .is_some()
        );
    }

    fn append_boolean(
        builder: &mut evrel_ir::FunctionBuilder<'_>,
        value: bool,
    ) -> evrel_ir::ValueId {
        let operation = builder.append_operation(
            OperationKind::Constant(ConstantOp::new(ConstantValue::Boolean(value))),
            [],
            UnwindTarget::Propagate,
        );

        builder.operation_results(operation)[0]
    }

    fn append_number(builder: &mut evrel_ir::FunctionBuilder<'_>, value: f64) -> evrel_ir::ValueId {
        let operation = builder.append_operation(
            OperationKind::Constant(ConstantOp::new(ConstantValue::Number(value))),
            [],
            UnwindTarget::Propagate,
        );

        builder.operation_results(operation)[0]
    }

    fn successor(
        function: &evrel_ir::FunctionIr,
        block: evrel_ir::BlockId,
    ) -> (evrel_ir::BlockId, Vec<evrel_ir::ValueId>) {
        let terminator = function.block(block).unwrap().terminator().unwrap();
        let operation = function.operation(terminator).unwrap();
        let successor = operation.successors()[0];

        (
            successor.target().block(),
            successor.arguments(operation.operands()).to_vec(),
        )
    }
}
