use super::*;

pub(super) fn plan_sequence<'function>(
    context: &ControlPlanningContext<'function>,
    locals: &mut JsLocalAllocator,
    start: BlockId,
    stop: Option<BlockId>,
    visited: &mut HashSet<BlockId>,
    active_controls: &[ActiveControl<'function>],
) -> Result<JsControlSequence, JsCodegenError> {
    let function_id = context.function_id;
    let function = context.function;
    let structures = &context.structures;

    let mut steps = Vec::new();
    let mut current = start;

    loop {
        if Some(current) == stop {
            break;
        }

        if let Some(statement) = structures.labeled_statement_at(current)
            && !active_controls
                .iter()
                .any(|control| control.structure_entry == current)
        {
            let completion = statement.completion_block();
            let mut nested_controls = active_controls.to_vec();

            nested_controls.push(ActiveControl {
                structure_entry: current,
                produced_block: None,
                continue_target: None,
                break_target: completion,
                label: statement.labels().last().map(Box::as_ref),
                completion_flag: None,
            });

            let body = plan_sequence(
                context,
                locals,
                current,
                Some(completion),
                visited,
                &nested_controls,
            )?;

            steps.push(JsControlStep::Labeled {
                labels: statement.labels().into(),
                body,
            });
            current = completion;
            continue;
        }

        if let Some(loop_operation) = structures.loop_at(current)
            && !active_controls
                .iter()
                .any(|control| control.structure_entry == current)
        {
            let (step, exit) = match loop_operation {
                LoopOperation::While {
                    operation_block,
                    operation,
                } => plan_while(
                    context,
                    locals,
                    current,
                    operation_block,
                    operation,
                    visited,
                    active_controls,
                )?,
                LoopOperation::DoWhile {
                    operation_block,
                    operation,
                } => plan_do_while(
                    context,
                    locals,
                    current,
                    operation_block,
                    operation,
                    visited,
                    active_controls,
                )?,
                LoopOperation::For {
                    operation_block,
                    operation,
                } => plan_for(
                    context,
                    locals,
                    current,
                    operation_block,
                    operation,
                    visited,
                    active_controls,
                )?,
                LoopOperation::ForIn {
                    operation_block,
                    operation,
                } if operation_block == current => plan_iterator(
                    context,
                    locals,
                    current,
                    IteratorOperation::ForIn(operation),
                    visited,
                    active_controls,
                )?,
                LoopOperation::ForOf {
                    operation_block,
                    operation,
                } if operation_block == current => plan_iterator(
                    context,
                    locals,
                    current,
                    IteratorOperation::ForOf(operation),
                    visited,
                    active_controls,
                )?,
                LoopOperation::ForIn { .. } | LoopOperation::ForOf { .. } => {
                    return Err(JsCodegenError::UnsupportedControlFlow {
                        function: function_id,
                        reason: concat!(file!(), ":", line!()),
                    });
                }
            };

            steps.push(step);
            current = exit;
            continue;
        }

        if !visited.insert(current) {
            return Err(JsCodegenError::UnsupportedControlFlow {
                function: function_id,
                reason: concat!(file!(), ":", line!()),
            });
        }

        let block = function
            .block(current)
            .ok_or(JsCodegenError::UnknownBlock { block: current })?;

        if block
            .parameters()
            .iter()
            .any(|parameter| match parameter.source() {
                BlockParameterSource::Forwarded => false,
                BlockParameterSource::Produced => !active_controls
                    .iter()
                    .any(|control| control.produced_block == Some(current)),
                BlockParameterSource::Exception => !structures.is_exception_entry(current),
            })
        {
            return Err(JsCodegenError::UnsupportedControlFlow {
                function: function_id,
                reason: concat!(file!(), ":", line!()),
            });
        }

        steps.push(JsControlStep::Block(current));

        let Some(terminator_id) = block.terminator() else {
            if stop.is_some() {
                return Err(JsCodegenError::UnsupportedControlFlow {
                    function: function_id,
                    reason: concat!(file!(), ":", line!()),
                });
            }

            break;
        };

        let terminator =
            function
                .operation(terminator_id)
                .ok_or(JsCodegenError::UnknownOperation {
                    operation: terminator_id,
                })?;

        match terminator.kind() {
            OperationKind::Jump(jump) => {
                if terminator.operands().len() != jump.target().argument_count() {
                    return Err(JsCodegenError::UnsupportedControlFlow {
                        function: function_id,
                        reason: concat!(file!(), ":", line!()),
                    });
                }

                steps.push(JsControlStep::Edge(JsEdgeKey::new(terminator_id, 0)));
                let target = jump.target().block();

                if Some(target) == stop {
                    current = target;
                    continue;
                }

                if let Some(transfer) = structured_transfer(target, active_controls) {
                    steps.push(transfer);
                    break;
                }

                current = target;
            }

            OperationKind::If(branch) => {
                let Some(condition) = terminator.operands().first().copied() else {
                    return Err(JsCodegenError::UnsupportedControlFlow {
                        function: function_id,
                        reason: concat!(file!(), ":", line!()),
                    });
                };
                let expected_operands = 1
                    + branch.then_target().argument_count()
                    + branch.else_target().argument_count();
                if terminator.operands().len() != expected_operands {
                    return Err(JsCodegenError::UnsupportedControlFlow {
                        function: function_id,
                        reason: concat!(file!(), ":", line!()),
                    });
                }

                let completion = branch.completion_block();
                let then_branch = plan_successor_sequence(
                    context,
                    locals,
                    JsEdgeKey::new(terminator_id, 0),
                    branch.then_target().block(),
                    Some(completion),
                    visited,
                    active_controls,
                )?;
                let else_branch = plan_successor_sequence(
                    context,
                    locals,
                    JsEdgeKey::new(terminator_id, 1),
                    branch.else_target().block(),
                    Some(completion),
                    visited,
                    active_controls,
                )?;

                steps.push(JsControlStep::If {
                    condition,
                    then_branch,
                    else_branch,
                });

                current = completion;
            }

            OperationKind::Switch(operation) => {
                let completion = operation.completion_block();
                steps.push(JsControlStep::Switch(plan_switch(
                    context,
                    locals,
                    current,
                    terminator_id,
                    operation,
                    visited,
                    active_controls,
                )?));
                current = completion;
            }

            OperationKind::Try(operation) => {
                let completion = operation.completion_block();
                steps.push(JsControlStep::Try(plan_try(
                    context,
                    locals,
                    terminator_id,
                    operation,
                    visited,
                    active_controls,
                )?));
                current = completion;
            }

            OperationKind::Return(_) | OperationKind::Throw(_) => break,

            _ => {
                return Err(JsCodegenError::UnsupportedControlFlow {
                    function: function_id,
                    reason: concat!(file!(), ":", line!()),
                });
            }
        }
    }

    Ok(JsControlSequence { steps })
}

pub(super) fn plan_successor_sequence<'function>(
    context: &ControlPlanningContext<'function>,
    locals: &mut JsLocalAllocator,
    edge: JsEdgeKey,
    target: BlockId,
    stop: Option<BlockId>,
    visited: &mut HashSet<BlockId>,
    active_controls: &[ActiveControl<'function>],
) -> Result<JsControlSequence, JsCodegenError> {
    if let Some(transfer) = structured_transfer(target, active_controls) {
        return Ok(JsControlSequence {
            steps: vec![JsControlStep::Edge(edge), transfer],
        });
    }

    let mut sequence = plan_sequence(context, locals, target, stop, visited, active_controls)?;
    sequence.prepend_edge(edge);

    Ok(sequence)
}
