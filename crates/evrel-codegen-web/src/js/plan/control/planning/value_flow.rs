use super::*;

pub(super) fn sequence_contains_var_destructure_binding(
    function: &JsFunctionIr,
    sequence: &JsControlSequence,
) -> bool {
    sequence.steps().iter().any(|step| match step {
        JsControlStep::Operations(operations) => operations.iter().any(|operation| {
            function.operation(*operation).is_some_and(|operation| {
                matches!(
                    operation.kind(),
                    OperationKind::DestructureBinding(destructure)
                        if destructure.mode() == evrel_js_ir::BindingWriteMode::Store
                )
            })
        }),
        JsControlStep::If {
            then_branch,
            else_branch,
            ..
        } => {
            sequence_contains_var_destructure_binding(function, then_branch)
                || sequence_contains_var_destructure_binding(function, else_branch)
        }
        _ => false,
    })
}

pub(super) fn sequence_contains_binding_declaration(
    function: &JsFunctionIr,
    sequence: &JsControlSequence,
) -> bool {
    sequence.steps().iter().any(|step| match step {
        JsControlStep::Operations(operations) => operations.iter().any(|operation| {
            function.operation(*operation).is_some_and(|operation| {
                matches!(
                    operation.kind(),
                    OperationKind::InitializeBinding(_) | OperationKind::DestructureBinding(_)
                )
            })
        }),
        JsControlStep::If {
            then_branch,
            else_branch,
            ..
        } => {
            sequence_contains_binding_declaration(function, then_branch)
                || sequence_contains_binding_declaration(function, else_branch)
        }
        _ => false,
    })
}

pub(super) fn recognize_for_test_flow(
    function: &JsFunctionIr,
    entry: BlockId,
    body: BlockId,
    exit: BlockId,
    active_exception_target: Option<BlockId>,
) -> Option<(JsForTestPlan, Vec<BlockId>)> {
    let completion = find_for_test_completion(function, entry, body, exit)?;
    if completion == entry || completion == body || completion == exit {
        return None;
    }

    let completion_block = function.block(completion)?;
    if !value_flow_parameters_are_supported(function, completion, completion_block.parameters()) {
        return None;
    }
    let completion_terminator_id = completion_block.terminator()?;
    let completion_terminator = function.operation(completion_terminator_id)?;
    let OperationKind::If(final_branch) = completion_terminator.kind() else {
        return None;
    };
    if final_branch.then_target().block() != body || final_branch.else_target().block() != exit {
        return None;
    }
    let condition = *completion_terminator.operands().first()?;
    let expected_operands = 1
        + final_branch.then_target().argument_count()
        + final_branch.else_target().argument_count();
    if completion_terminator.operands().len() != expected_operands {
        return None;
    }

    let (test_flow, mut blocks, edges) =
        plan_value_flow(function, entry, completion, active_exception_target)?;
    if !blocks.contains(&completion) {
        blocks.push(completion);
    }
    Some((
        JsForTestPlan::Flow {
            value_flow: JsValueFlowPlan::new(
                test_flow,
                completion,
                condition,
                edges.into_boxed_slice(),
            ),
            body_edge: JsEdgeKey::new(completion_terminator_id, 0),
            exit_edge: JsEdgeKey::new(completion_terminator_id, 1),
        },
        blocks,
    ))
}

pub(super) fn find_for_test_completion(
    function: &JsFunctionIr,
    entry: BlockId,
    body: BlockId,
    exit: BlockId,
) -> Option<BlockId> {
    let mut pending = vec![entry];
    let mut seen = HashSet::new();
    let mut completion = None;

    while let Some(block) = pending.pop() {
        if block == body || block == exit || !seen.insert(block) {
            continue;
        }
        let block_data = function.block(block)?;
        let terminator = function.operation(block_data.terminator()?)?;
        if let OperationKind::If(branch) = terminator.kind()
            && branch.then_target().block() == body
            && branch.else_target().block() == exit
        {
            if completion.replace(block).is_some() {
                return None;
            }
            continue;
        }
        if matches!(terminator.kind(), OperationKind::Invoke(_)) {
            let normal = terminator.successors().first().copied()?;
            pending.push(normal.target().block());
            continue;
        }

        match terminator.kind() {
            OperationKind::Jump(jump) => pending.push(jump.target().block()),
            OperationKind::If(branch) => {
                pending.push(branch.then_target().block());
                pending.push(branch.else_target().block());
            }
            _ => return None,
        }
    }

    completion
}

pub(super) fn plan_value_flow(
    function: &JsFunctionIr,
    entry: BlockId,
    completion: BlockId,
    active_exception_target: Option<BlockId>,
) -> Option<(JsValueFlowStep, Vec<BlockId>, Vec<JsEdgeKey>)> {
    let mut blocks = Vec::new();
    let mut edges = Vec::new();
    let root = plan_value_flow_step(
        function,
        entry,
        completion,
        &mut blocks,
        &mut edges,
        &mut HashSet::new(),
        active_exception_target,
    )?;

    Some((root, blocks, edges))
}

pub(super) fn plan_value_flow_step(
    function: &JsFunctionIr,
    block: BlockId,
    completion: BlockId,
    blocks: &mut Vec<BlockId>,
    edges: &mut Vec<JsEdgeKey>,
    active: &mut HashSet<BlockId>,
    active_exception_target: Option<BlockId>,
) -> Option<JsValueFlowStep> {
    if block == completion {
        return Some(JsValueFlowStep::Complete);
    }
    if !active.insert(block) {
        return None;
    }
    if !blocks.contains(&block) {
        blocks.push(block);
    }
    let block_data = function.block(block)?;
    if !value_flow_parameters_are_supported(function, block, block_data.parameters()) {
        return None;
    }
    let terminator_id = block_data.terminator()?;
    let terminator = function.operation(terminator_id)?;
    let mut operations = block_data.operations().to_vec();
    let continuation = if matches!(terminator.kind(), OperationKind::Invoke(_)) {
        let successors = terminator.successors();
        let [normal, exception] = successors.as_slice() else {
            return None;
        };
        if Some(exception.target().block()) != active_exception_target {
            return None;
        }
        operations.push(terminator_id);
        let normal_edge = JsEdgeKey::new(terminator_id, 0);
        edges.push(normal_edge);
        let next = plan_value_flow_step(
            function,
            normal.target().block(),
            completion,
            blocks,
            edges,
            active,
            active_exception_target,
        )?;
        JsValueFlowContinuation::Jump {
            edge: normal_edge,
            next: Box::new(next),
        }
    } else {
        match terminator.kind() {
            OperationKind::Jump(jump) => {
                let edge = JsEdgeKey::new(terminator_id, 0);
                edges.push(edge);
                let next = plan_value_flow_step(
                    function,
                    jump.target().block(),
                    completion,
                    blocks,
                    edges,
                    active,
                    active_exception_target,
                )?;
                JsValueFlowContinuation::Jump {
                    edge,
                    next: Box::new(next),
                }
            }
            OperationKind::If(branch) => {
                let condition = *terminator.operands().first()?;
                let then_edge = JsEdgeKey::new(terminator_id, 0);
                let else_edge = JsEdgeKey::new(terminator_id, 1);
                edges.push(then_edge);
                edges.push(else_edge);
                let then_step = plan_value_flow_step(
                    function,
                    branch.then_target().block(),
                    completion,
                    blocks,
                    edges,
                    active,
                    active_exception_target,
                )?;
                let else_step = plan_value_flow_step(
                    function,
                    branch.else_target().block(),
                    completion,
                    blocks,
                    edges,
                    active,
                    active_exception_target,
                )?;
                JsValueFlowContinuation::Branch {
                    condition,
                    then_edge,
                    then_step: Box::new(then_step),
                    else_edge,
                    else_step: Box::new(else_step),
                }
            }
            _ => return None,
        }
    };
    active.remove(&block);
    Some(JsValueFlowStep::Operations {
        operations: operations.into_boxed_slice(),
        continuation,
    })
}

pub(super) fn value_flow_parameters_are_supported(
    function: &JsFunctionIr,
    block: BlockId,
    parameters: &[evrel_js_ir::BlockParameter],
) -> bool {
    let is_invoke_normal_entry = is_invoke_normal_entry(function, block);

    parameters.iter().all(|parameter| match parameter.source() {
        BlockParameterSource::Forwarded => true,
        BlockParameterSource::Produced => is_invoke_normal_entry,
        BlockParameterSource::Exception => false,
    })
}

fn is_invoke_normal_entry(function: &JsFunctionIr, block: BlockId) -> bool {
    function.operations().any(|(_, operation)| {
        matches!(
            operation.kind(),
            OperationKind::Invoke(invoke) if invoke.normal_target().block() == block
        )
    })
}
