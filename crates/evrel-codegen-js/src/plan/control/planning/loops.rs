use super::*;

use super::value_flow::*;

pub(super) fn plan_do_while<'function>(
    context: &ControlPlanningContext<'function>,
    locals: &mut JsLocalAllocator,
    entry: BlockId,
    operation_block: BlockId,
    loop_operation: &'function DoWhileOp,
    visited: &mut HashSet<BlockId>,
    active_controls: &[ActiveControl<'function>],
) -> Result<(JsControlStep, BlockId), JsCodegenError> {
    let function_id = context.function_id;
    let function = context.function;

    if entry != loop_operation.body_target().block() {
        return Err(JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        });
    }

    let test_block = loop_operation.test_block();
    let exit_block = loop_operation.exit_target().block();
    let mut nested_controls = active_controls.to_vec();

    nested_controls.push(ActiveControl {
        structure_entry: entry,
        produced_block: None,
        continue_target: Some(test_block),
        break_target: exit_block,
        label: loop_operation.labels().last().map(Box::as_ref),
        completion_flag: None,
    });

    let body = plan_sequence(
        context,
        locals,
        entry,
        Some(test_block),
        visited,
        &nested_controls,
    )?;

    let operation_block_data =
        function
            .block(operation_block)
            .ok_or(JsCodegenError::UnknownBlock {
                block: operation_block,
            })?;

    if operation_block_data
        .parameters()
        .iter()
        .any(|parameter| parameter.source() != BlockParameterSource::Forwarded)
    {
        return Err(JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        });
    }

    let terminator_id =
        operation_block_data
            .terminator()
            .ok_or(JsCodegenError::UnsupportedControlFlow {
                function: function_id,
                reason: concat!(file!(), ":", line!()),
            })?;
    let terminator = function
        .operation(terminator_id)
        .ok_or(JsCodegenError::UnknownOperation {
            operation: terminator_id,
        })?;
    let OperationKind::DoWhile(planned_operation) = terminator.kind() else {
        return Err(JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        });
    };

    if !std::ptr::eq(planned_operation, loop_operation) {
        return Err(JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        });
    }

    let Some(condition) = terminator.operands().first().copied() else {
        return Err(JsCodegenError::MalformedOperation {
            operation: terminator_id,
        });
    };
    let expected_operands = 1
        + loop_operation.body_target().argument_count()
        + loop_operation.exit_target().argument_count();

    if terminator.operands().len() != expected_operands {
        return Err(JsCodegenError::MalformedOperation {
            operation: terminator_id,
        });
    }

    let (test_flow, mut test_blocks, test_edges) =
        plan_value_flow(function, test_block, operation_block).ok_or(
            JsCodegenError::UnsupportedControlFlow {
                function: function_id,
                reason: concat!(file!(), ":", line!()),
            },
        )?;
    if !test_blocks.contains(&operation_block) {
        test_blocks.push(operation_block);
    }
    for &block in &test_blocks {
        if !visited.insert(block) {
            return Err(JsCodegenError::UnsupportedControlFlow {
                function: function_id,
                reason: concat!(file!(), ":", line!()),
            });
        }
    }
    Ok((
        JsControlStep::DoWhile {
            labels: loop_operation.labels().into(),
            body,
            test: JsFlowTestPlan::new(
                JsValueFlowPlan::new(
                    test_flow,
                    operation_block,
                    condition,
                    test_edges.into_boxed_slice(),
                ),
                JsEdgeKey::new(terminator_id, 0),
                JsEdgeKey::new(terminator_id, 1),
            ),
        },
        exit_block,
    ))
}

pub(super) fn plan_while<'function>(
    context: &ControlPlanningContext<'function>,
    locals: &mut JsLocalAllocator,
    entry: BlockId,
    operation_block: BlockId,
    loop_operation: &'function WhileOp,
    visited: &mut HashSet<BlockId>,
    active_controls: &[ActiveControl<'function>],
) -> Result<(JsControlStep, BlockId), JsCodegenError> {
    let function_id = context.function_id;
    let function = context.function;

    if entry != loop_operation.test_block() || !visited.insert(entry) {
        return Err(JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        });
    }

    if operation_block != entry {
        return plan_while_flow(
            context,
            locals,
            entry,
            operation_block,
            loop_operation,
            visited,
            active_controls,
        );
    }

    let test_block = function
        .block(entry)
        .ok_or(JsCodegenError::UnknownBlock { block: entry })?;

    if test_block
        .parameters()
        .iter()
        .any(|parameter| parameter.source() != BlockParameterSource::Forwarded)
    {
        return Err(JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        });
    }

    let terminator_id = test_block
        .terminator()
        .ok_or(JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        })?;
    let terminator = function
        .operation(terminator_id)
        .ok_or(JsCodegenError::UnknownOperation {
            operation: terminator_id,
        })?;
    let OperationKind::While(planned_operation) = terminator.kind() else {
        return Err(JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        });
    };

    if !std::ptr::eq(planned_operation, loop_operation) {
        return Err(JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        });
    }

    let Some(condition) = terminator.operands().first().copied() else {
        return Err(JsCodegenError::MalformedOperation {
            operation: terminator_id,
        });
    };
    let expected_operands = 1
        + loop_operation.body_target().argument_count()
        + loop_operation.exit_target().argument_count();

    if terminator.operands().len() != expected_operands {
        return Err(JsCodegenError::MalformedOperation {
            operation: terminator_id,
        });
    }

    let exit = loop_operation.exit_target().block();
    let mut nested_controls = active_controls.to_vec();

    nested_controls.push(ActiveControl {
        structure_entry: entry,
        produced_block: None,
        continue_target: Some(entry),
        break_target: exit,
        label: loop_operation.labels().last().map(Box::as_ref),
        completion_flag: None,
    });

    let body = plan_sequence(
        context,
        locals,
        loop_operation.body_target().block(),
        Some(entry),
        visited,
        &nested_controls,
    )?;

    Ok((
        JsControlStep::While {
            labels: loop_operation.labels().into(),
            test_block: entry,
            condition,
            body_edge: JsEdgeKey::new(terminator_id, 0),
            body,
            exit_edge: JsEdgeKey::new(terminator_id, 1),
        },
        exit,
    ))
}

pub(super) fn plan_while_flow<'function>(
    context: &ControlPlanningContext<'function>,
    locals: &mut JsLocalAllocator,
    entry: BlockId,
    operation_block: BlockId,
    loop_operation: &'function WhileOp,
    visited: &mut HashSet<BlockId>,
    active_controls: &[ActiveControl<'function>],
) -> Result<(JsControlStep, BlockId), JsCodegenError> {
    let function_id = context.function_id;
    let function = context.function;

    let operation_block_data =
        function
            .block(operation_block)
            .ok_or(JsCodegenError::UnknownBlock {
                block: operation_block,
            })?;
    let terminator_id =
        operation_block_data
            .terminator()
            .ok_or(JsCodegenError::UnsupportedControlFlow {
                function: function_id,
                reason: concat!(file!(), ":", line!()),
            })?;
    let terminator = function
        .operation(terminator_id)
        .ok_or(JsCodegenError::UnknownOperation {
            operation: terminator_id,
        })?;
    let OperationKind::While(actual) = terminator.kind() else {
        return Err(JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        });
    };
    if !std::ptr::eq(actual, loop_operation) {
        return Err(JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        });
    }
    let condition = *terminator
        .operands()
        .first()
        .ok_or(JsCodegenError::MalformedOperation {
            operation: terminator_id,
        })?;
    let expected_operands = 1
        + loop_operation.body_target().argument_count()
        + loop_operation.exit_target().argument_count();
    if terminator.operands().len() != expected_operands {
        return Err(JsCodegenError::MalformedOperation {
            operation: terminator_id,
        });
    }

    let (test_flow, mut blocks, edges) = plan_value_flow(function, entry, operation_block).ok_or(
        JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        },
    )?;
    if !blocks.contains(&operation_block) {
        blocks.push(operation_block);
    }
    for &block in &blocks {
        if block != entry && !visited.insert(block) {
            return Err(JsCodegenError::UnsupportedControlFlow {
                function: function_id,
                reason: concat!(file!(), ":", line!()),
            });
        }
    }
    let exit = loop_operation.exit_target().block();
    let mut nested_controls = active_controls.to_vec();
    nested_controls.push(ActiveControl {
        structure_entry: entry,
        produced_block: None,
        continue_target: Some(entry),
        break_target: exit,
        label: loop_operation.labels().last().map(Box::as_ref),
        completion_flag: None,
    });
    let body = plan_sequence(
        context,
        locals,
        loop_operation.body_target().block(),
        Some(entry),
        visited,
        &nested_controls,
    )?;

    Ok((
        JsControlStep::WhileFlow {
            labels: loop_operation.labels().into(),
            test: JsFlowTestPlan::new(
                JsValueFlowPlan::new(
                    test_flow,
                    operation_block,
                    condition,
                    edges.into_boxed_slice(),
                ),
                JsEdgeKey::new(terminator_id, 0),
                JsEdgeKey::new(terminator_id, 1),
            ),
            body,
        },
        exit,
    ))
}

pub(super) fn plan_for<'function>(
    context: &ControlPlanningContext<'function>,
    locals: &mut JsLocalAllocator,
    entry: BlockId,
    operation_block: BlockId,
    loop_operation: &'function ForOp,
    visited: &mut HashSet<BlockId>,
    active_controls: &[ActiveControl<'function>],
) -> Result<(JsControlStep, BlockId), JsCodegenError> {
    let function_id = context.function_id;
    let function = context.function;

    if entry != loop_operation.initializer_block() {
        return Err(JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        });
    }

    let test_block = loop_operation.test_target().block();
    let body_block = loop_operation.body_block();
    let update_block = loop_operation.update_block();
    let exit_block = loop_operation.exit_block();
    let mut initializer_controls = active_controls.to_vec();
    initializer_controls.push(ActiveControl {
        structure_entry: entry,
        produced_block: None,
        continue_target: None,
        break_target: exit_block,
        label: loop_operation.labels().last().map(Box::as_ref),
        completion_flag: None,
    });
    let initializer = plan_sequence(
        context,
        locals,
        entry,
        Some(operation_block),
        visited,
        &initializer_controls,
    )?;
    let initializer_has_control = initializer
        .steps()
        .iter()
        .any(|step| !matches!(step, JsControlStep::Block(_) | JsControlStep::Edge(_)));
    let initializer_is_prelude =
        initializer_has_control && !sequence_contains_binding_declaration(function, &initializer);

    if !visited.insert(operation_block) {
        return Err(JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        });
    }

    let host_block = function
        .block(operation_block)
        .ok_or(JsCodegenError::UnknownBlock {
            block: operation_block,
        })?;
    let host_terminator_id =
        host_block
            .terminator()
            .ok_or(JsCodegenError::UnsupportedControlFlow {
                function: function_id,
                reason: concat!(file!(), ":", line!()),
            })?;
    let host_terminator =
        function
            .operation(host_terminator_id)
            .ok_or(JsCodegenError::UnknownOperation {
                operation: host_terminator_id,
            })?;
    let OperationKind::For(planned_operation) = host_terminator.kind() else {
        return Err(JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        });
    };

    if !std::ptr::eq(planned_operation, loop_operation)
        || host_terminator.operands().len() != loop_operation.test_target().argument_count()
    {
        return Err(JsCodegenError::MalformedOperation {
            operation: host_terminator_id,
        });
    }

    let mut nested_controls = active_controls.to_vec();
    nested_controls.push(ActiveControl {
        structure_entry: entry,
        produced_block: None,
        continue_target: Some(update_block),
        break_target: exit_block,
        label: loop_operation.labels().last().map(Box::as_ref),
        completion_flag: None,
    });

    let body = plan_sequence(
        context,
        locals,
        body_block,
        Some(update_block),
        visited,
        &nested_controls,
    )?;
    let update = plan_sequence(
        context,
        locals,
        update_block,
        Some(operation_block),
        visited,
        &nested_controls,
    )?;
    if !visited.insert(test_block) {
        return Err(JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        });
    }

    let test_block_data = function
        .block(test_block)
        .ok_or(JsCodegenError::UnknownBlock { block: test_block })?;

    if test_block_data
        .parameters()
        .iter()
        .any(|parameter| parameter.source() != BlockParameterSource::Forwarded)
    {
        return Err(JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        });
    }

    let test_terminator_id =
        test_block_data
            .terminator()
            .ok_or(JsCodegenError::UnsupportedControlFlow {
                function: function_id,
                reason: concat!(file!(), ":", line!()),
            })?;
    let test_terminator =
        function
            .operation(test_terminator_id)
            .ok_or(JsCodegenError::UnknownOperation {
                operation: test_terminator_id,
            })?;
    let test = match test_terminator.kind() {
        OperationKind::Jump(jump)
            if jump.target().block() == body_block
                && test_terminator.operands().len() == jump.target().argument_count() =>
        {
            JsForTestPlan::Always {
                block: test_block,
                body_edge: JsEdgeKey::new(test_terminator_id, 0),
            }
        }
        OperationKind::If(branch)
            if branch.then_target().block() == body_block
                && branch.else_target().block() == exit_block =>
        {
            let Some(condition) = test_terminator.operands().first().copied() else {
                return Err(JsCodegenError::MalformedOperation {
                    operation: test_terminator_id,
                });
            };
            let expected_operands =
                1 + branch.then_target().argument_count() + branch.else_target().argument_count();

            if test_terminator.operands().len() != expected_operands {
                return Err(JsCodegenError::MalformedOperation {
                    operation: test_terminator_id,
                });
            }

            JsForTestPlan::Conditional {
                block: test_block,
                condition,
                body_edge: JsEdgeKey::new(test_terminator_id, 0),
                exit_edge: JsEdgeKey::new(test_terminator_id, 1),
            }
        }
        _ => {
            let Some((flow, flow_blocks)) =
                recognize_for_test_flow(function, test_block, body_block, exit_block)
            else {
                return Err(JsCodegenError::UnsupportedControlFlow {
                    function: function_id,
                    reason: concat!(file!(), ":", line!()),
                });
            };
            for block in flow_blocks {
                if block != test_block && !visited.insert(block) {
                    return Err(JsCodegenError::UnsupportedControlFlow {
                        function: function_id,
                        reason: concat!(file!(), ":", line!()),
                    });
                }
            }
            flow
        }
    };

    Ok((
        JsControlStep::For(JsForPlan {
            labels: loop_operation.labels().into(),
            initializer,
            initializer_is_prelude,
            enter_test_edge: JsEdgeKey::new(host_terminator_id, 0),
            test,
            body,
            update,
        }),
        exit_block,
    ))
}
