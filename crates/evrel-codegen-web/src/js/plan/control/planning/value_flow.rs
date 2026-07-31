use super::*;

pub(super) fn sequence_contains_binding_declaration(
    function: &JsFunctionIr,
    sequence: &JsControlSequence,
) -> bool {
    sequence.steps().iter().any(|step| match step {
        JsControlStep::Block(block) => function.block(*block).is_some_and(|block| {
            block.operations().iter().any(|operation| {
                function.operation(*operation).is_some_and(|operation| {
                    matches!(
                        operation.kind(),
                        OperationKind::InitializeBinding(_) | OperationKind::DestructureBinding(_)
                    )
                })
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
) -> Option<(JsForTestPlan, Vec<BlockId>)> {
    let completion = find_for_test_completion(function, entry, body, exit)?;
    if completion == entry || completion == body || completion == exit {
        return None;
    }

    let completion_block = function.block(completion)?;
    if completion_block
        .parameters()
        .iter()
        .any(|parameter| parameter.source() != BlockParameterSource::Forwarded)
    {
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

    let (test_flow, mut blocks, edges) = plan_value_flow(function, entry, completion)?;
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
    if block_data
        .parameters()
        .iter()
        .any(|parameter| parameter.source() != BlockParameterSource::Forwarded)
    {
        return None;
    }
    let terminator_id = block_data.terminator()?;
    let terminator = function.operation(terminator_id)?;
    let continuation = match terminator.kind() {
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
            )?;
            let else_step = plan_value_flow_step(
                function,
                branch.else_target().block(),
                completion,
                blocks,
                edges,
                active,
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
    };
    active.remove(&block);
    Some(JsValueFlowStep::Block {
        block,
        continuation,
    })
}
