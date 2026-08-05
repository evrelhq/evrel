use super::*;

pub(super) enum IteratorOperation<'operation> {
    ForIn(&'operation ForInOp),
    ForOf(&'operation ForOfOp),
}

impl IteratorOperation<'_> {
    fn kind(&self) -> JsIteratorKind {
        match self {
            Self::ForIn(_) => JsIteratorKind::In,
            Self::ForOf(operation) if operation.kind() == ForOfKind::Asynchronous => {
                JsIteratorKind::AwaitOf
            }
            Self::ForOf(_) => JsIteratorKind::Of,
        }
    }

    fn body_target(&self) -> evrel_js_ir::BlockTarget {
        match self {
            Self::ForIn(operation) => operation.body_target(),
            Self::ForOf(operation) => operation.body_target(),
        }
    }

    fn exit_target(&self) -> evrel_js_ir::BlockTarget {
        match self {
            Self::ForIn(operation) => operation.exit_target(),
            Self::ForOf(operation) => operation.exit_target(),
        }
    }

    fn labels(&self) -> &[Box<str>] {
        match self {
            Self::ForIn(operation) => operation.labels(),
            Self::ForOf(operation) => operation.labels(),
        }
    }
}

pub(super) fn produced_parameter_local(
    function_id: FunctionId,
    function: &JsFunctionIr,
    values: &DenseMap<ValueId, JsValueRepresentation>,
    body_block: BlockId,
) -> Result<JsLocalId, JsCodegenError> {
    let body = function
        .block(body_block)
        .ok_or(JsCodegenError::UnknownBlock { block: body_block })?;
    let Some(produced) = body.parameters().first() else {
        return Err(JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        });
    };

    if produced.source() != BlockParameterSource::Produced
        || body.parameters()[1..]
            .iter()
            .any(|parameter| parameter.source() != BlockParameterSource::Forwarded)
    {
        return Err(JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        });
    }

    let value = produced.value();
    let Some(JsValueRepresentation::Temporary(local)) = values.get(value).copied() else {
        return Err(JsCodegenError::UnsupportedValue { value });
    };

    Ok(local)
}

pub(super) fn plan_iterator<'function>(
    context: &ControlPlanningContext<'function>,
    locals: &mut JsLocalAllocator,
    header: BlockId,
    operation: IteratorOperation<'function>,
    visited: &mut HashSet<BlockId>,
    scope: ControlPlanningScope<'_, 'function>,
) -> Result<(JsControlStep, BlockId), JsCodegenError> {
    let function_id = context.function_id;
    let function = context.function;
    let values = context.values;

    if !visited.insert(header) {
        return Err(JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        });
    }

    let header_block = function
        .block(header)
        .ok_or(JsCodegenError::UnknownBlock { block: header })?;

    if header_block
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
        header_block
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
    let operation_matches = match (terminator.kind(), &operation) {
        (OperationKind::ForIn(actual), IteratorOperation::ForIn(expected)) => {
            std::ptr::eq(actual, *expected)
        }
        (OperationKind::ForOf(actual), IteratorOperation::ForOf(expected)) => {
            std::ptr::eq(actual, *expected)
        }
        _ => false,
    };

    if !operation_matches {
        return Err(JsCodegenError::UnsupportedControlFlow {
            function: function_id,
            reason: concat!(file!(), ":", line!()),
        });
    }

    let Some(iterated_value) = terminator.operands().first().copied() else {
        return Err(JsCodegenError::MalformedOperation {
            operation: terminator_id,
        });
    };
    let body_target = operation.body_target();
    let exit_target = operation.exit_target();
    let expected_operands = 1 + body_target.argument_count() + exit_target.argument_count();

    if terminator.operands().len() != expected_operands {
        return Err(JsCodegenError::MalformedOperation {
            operation: terminator_id,
        });
    }

    let body_block = body_target.block();
    let exit_block = exit_target.block();
    let produced_local = produced_parameter_local(function_id, function, values, body_block)?;
    let completion_flag = locals.allocate();
    let mut nested_controls = scope.active_controls.to_vec();

    nested_controls.push(ActiveControl {
        structure_entry: header,
        produced_block: Some(body_block),
        continue_target: Some(header),
        break_target: exit_block,
        label: operation.labels().last().map(Box::as_ref),
        completion_flag: Some(completion_flag),
    });

    let mut body = plan_sequence(
        context,
        locals,
        body_block,
        Some(header),
        visited,
        scope.with_controls(&nested_controls),
    )?;
    body.prepend_edge(JsEdgeKey::new(terminator_id, 0));

    Ok((
        JsControlStep::Iterator(JsIteratorPlan {
            kind: operation.kind(),
            labels: operation.labels().into(),
            iterated_value,
            produced_local,
            completion_flag,
            body,
            natural_exit_edge: JsEdgeKey::new(terminator_id, 1),
        }),
        exit_block,
    ))
}
