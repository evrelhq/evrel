use super::*;

pub(super) fn plan_sequence<'function>(
    context: &ControlPlanningContext<'function>,
    locals: &mut JsLocalAllocator,
    start: BlockId,
    stop: Option<BlockId>,
    visited: &mut HashSet<BlockId>,
    scope: ControlPlanningScope<'_, 'function>,
) -> Result<JsControlSequence, JsCodegenError> {
    let function_id = context.function_id;
    let function = context.function;
    let structures = &context.structures;
    let active_controls = scope.active_controls;
    let active_exception_target = scope.exception_target;

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
                scope.with_controls(&nested_controls),
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
                    scope,
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
                    scope,
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
                    scope,
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
                    scope,
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
                    scope,
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
                BlockParameterSource::Produced => {
                    !structures.is_invoke_normal_entry(current)
                        && !structures.is_completion_entry(current)
                        && !active_controls
                            .iter()
                            .any(|control| control.produced_block == Some(current))
                }
                BlockParameterSource::Exception => !structures.is_exception_entry(current),
            })
        {
            return Err(JsCodegenError::UnsupportedControlFlow {
                function: function_id,
                reason: concat!(file!(), ":", line!()),
            });
        }

        steps.push(JsControlStep::Operations(
            block.operations().to_vec().into_boxed_slice(),
        ));

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

        if matches!(terminator.kind(), OperationKind::Invoke(_)) {
            let successors = terminator.successors();
            let [normal, exception] = successors.as_slice() else {
                return Err(JsCodegenError::MalformedOperation {
                    operation: terminator_id,
                });
            };

            if Some(exception.target().block()) != active_exception_target {
                return Err(JsCodegenError::UnsupportedControlFlow {
                    function: function_id,
                    reason: concat!(file!(), ":", line!()),
                });
            }

            let normal_edge = JsEdgeKey::new(terminator_id, 0);
            steps.push(JsControlStep::Operations(Box::new([terminator_id])));
            steps.push(JsControlStep::Edge(normal_edge));
            let target = normal.target().block();

            if Some(target) == stop {
                current = target;
                continue;
            }

            if let Some(transfer) = structured_transfer(target, active_controls) {
                steps.push(transfer);
                break;
            }

            current = target;
            continue;
        }

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
                    scope,
                )?;
                let else_branch = plan_successor_sequence(
                    context,
                    locals,
                    JsEdgeKey::new(terminator_id, 1),
                    branch.else_target().block(),
                    Some(completion),
                    visited,
                    scope,
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
                    scope,
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
                    scope,
                )?));
                current = completion;
            }

            OperationKind::EnterFinally(operation) => {
                let successors = terminator.successors();
                let [successor] = successors.as_slice() else {
                    return Err(JsCodegenError::MalformedOperation {
                        operation: terminator_id,
                    });
                };

                if !structures.is_completion_entry(successor.target().block())
                    || successor.target().argument_count() != 0
                {
                    return Err(JsCodegenError::UnsupportedControlFlow {
                        function: function_id,
                        reason: concat!(file!(), ":", line!()),
                    });
                }

                match operation.kind() {
                    evrel_js_ir::CompletionKind::Normal => {}
                    evrel_js_ir::CompletionKind::Return => {
                        let [value] = terminator.operation_operands() else {
                            return Err(JsCodegenError::MalformedOperation {
                                operation: terminator_id,
                            });
                        };
                        steps.push(JsControlStep::Return { value: *value });
                    }
                    evrel_js_ir::CompletionKind::Throw => {
                        let [value] = terminator.operation_operands() else {
                            return Err(JsCodegenError::MalformedOperation {
                                operation: terminator_id,
                            });
                        };
                        steps.push(JsControlStep::Throw { value: *value });
                    }
                    evrel_js_ir::CompletionKind::Break(target) => {
                        let Some(transfer @ JsControlStep::Break { .. }) =
                            structured_transfer(target, active_controls)
                        else {
                            return Err(JsCodegenError::UnsupportedControlFlow {
                                function: function_id,
                                reason: concat!(file!(), ":", line!()),
                            });
                        };
                        steps.push(transfer);
                    }
                    evrel_js_ir::CompletionKind::Continue(target) => {
                        let Some(transfer @ JsControlStep::Continue { .. }) =
                            structured_transfer(target, active_controls)
                        else {
                            return Err(JsCodegenError::UnsupportedControlFlow {
                                function: function_id,
                                reason: concat!(file!(), ":", line!()),
                            });
                        };
                        steps.push(transfer);
                    }
                }

                break;
            }

            OperationKind::ResumeCompletion(operation) => {
                let Some(completion) = stop else {
                    return Err(JsCodegenError::UnsupportedControlFlow {
                        function: function_id,
                        reason: concat!(file!(), ":", line!()),
                    });
                };
                let mut has_normal_case = false;

                for case in operation.cases() {
                    if case.kind() == evrel_js_ir::CompletionKind::Normal {
                        if case.target().block() != completion {
                            return Err(JsCodegenError::UnsupportedControlFlow {
                                function: function_id,
                                reason: concat!(file!(), ":", line!()),
                            });
                        }
                        has_normal_case = true;
                    }
                }

                if !has_normal_case {
                    return Err(JsCodegenError::MalformedOperation {
                        operation: terminator_id,
                    });
                }

                break;
            }

            OperationKind::Throw(operation) => {
                if operation.exception_target().is_some() {
                    let successors = terminator.successors();
                    let [exception] = successors.as_slice() else {
                        return Err(JsCodegenError::MalformedOperation {
                            operation: terminator_id,
                        });
                    };

                    if Some(exception.target().block()) != active_exception_target {
                        return Err(JsCodegenError::UnsupportedControlFlow {
                            function: function_id,
                            reason: concat!(file!(), ":", line!()),
                        });
                    }
                }

                steps.push(JsControlStep::Operations(Box::new([terminator_id])));
                break;
            }

            OperationKind::Return(_) => {
                if !matches!(
                    function.kind(),
                    evrel_js_ir::FunctionKind::Module | evrel_js_ir::FunctionKind::ClassStaticBlock
                ) {
                    steps.push(JsControlStep::Operations(Box::new([terminator_id])));
                }
                break;
            }

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
    scope: ControlPlanningScope<'_, 'function>,
) -> Result<JsControlSequence, JsCodegenError> {
    if let Some(transfer) = structured_transfer(target, scope.active_controls) {
        return Ok(JsControlSequence {
            steps: vec![JsControlStep::Edge(edge), transfer],
        });
    }

    let mut sequence = plan_sequence(context, locals, target, stop, visited, scope)?;
    sequence.prepend_edge(edge);

    Ok(sequence)
}
