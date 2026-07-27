use super::*;

pub(super) fn plan_try(
    context: &ControlPlanningContext<'_>,
    locals: &mut JsLocalAllocator,
    terminator_id: OperationId,
    operation: &evrel_ir::TryOp,
    visited: &mut HashSet<BlockId>,
    active_controls: &[ActiveControl<'_>],
) -> Result<JsTryPlan, JsCodegenError> {
    let function_id = context.function_id;
    let function = context.function;

    let terminator = function
        .operation(terminator_id)
        .ok_or(JsCodegenError::UnknownOperation {
            operation: terminator_id,
        })?;

    if terminator.operands().len() != operation.try_target().argument_count() {
        return Err(JsCodegenError::MalformedOperation {
            operation: terminator_id,
        });
    }

    let normal_target = operation
        .finally_block()
        .unwrap_or(operation.completion_block());
    let mut try_body = plan_sequence(
        context,
        locals,
        operation.try_target().block(),
        Some(normal_target),
        visited,
        active_controls,
    )?;
    try_body.prepend_edge(JsEdgeKey::new(terminator_id, 0));

    let catch = operation
        .catch_block()
        .map(|catch_block| {
            let block = function
                .block(catch_block)
                .ok_or(JsCodegenError::UnknownBlock { block: catch_block })?;
            let Some(parameter) = block.parameters().first() else {
                return Err(JsCodegenError::UnsupportedControlFlow {
                    function: function_id,
                    reason: concat!(file!(), ":", line!()),
                });
            };

            if parameter.source() != BlockParameterSource::Exception {
                return Err(JsCodegenError::UnsupportedControlFlow {
                    function: function_id,
                    reason: concat!(file!(), ":", line!()),
                });
            }

            let body = plan_sequence(
                context,
                locals,
                catch_block,
                Some(normal_target),
                visited,
                active_controls,
            )?;

            Ok(JsCatchPlan::new(parameter.value(), body))
        })
        .transpose()?;

    let finally = operation
        .finally_block()
        .map(|finally_block| {
            plan_sequence(
                context,
                locals,
                finally_block,
                Some(operation.completion_block()),
                visited,
                active_controls,
            )
        })
        .transpose()?;

    Ok(JsTryPlan::new(try_body, catch, finally))
}
