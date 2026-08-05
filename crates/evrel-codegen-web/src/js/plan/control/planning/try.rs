use super::*;

pub(super) fn plan_try<'function>(
    context: &ControlPlanningContext<'function>,
    locals: &mut JsLocalAllocator,
    terminator_id: OperationId,
    operation: &evrel_js_ir::TryOp,
    visited: &mut HashSet<BlockId>,
    scope: ControlPlanningScope<'_, 'function>,
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
    let protected_exception_target = operation
        .catch_block()
        .or(operation.finally_exception_block())
        .or(scope.exception_target);
    let mut try_body = plan_sequence(
        context,
        locals,
        operation.try_target().block(),
        Some(normal_target),
        visited,
        scope.with_exception_target(protected_exception_target),
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
                scope.with_exception_target(
                    operation
                        .finally_exception_block()
                        .or(scope.exception_target),
                ),
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
                scope,
            )
        })
        .transpose()?;

    Ok(JsTryPlan::new(try_body, catch, finally))
}
