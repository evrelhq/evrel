use super::*;

pub(super) fn plan_switch<'function>(
    context: &ControlPlanningContext<'function>,
    locals: &mut JsLocalAllocator,
    switch_block: BlockId,
    terminator_id: OperationId,
    operation: &'function evrel_ir::SwitchOp,
    visited: &mut HashSet<BlockId>,
    active_controls: &[ActiveControl<'function>],
) -> Result<JsSwitchPlan, JsCodegenError> {
    let function = context.function;

    let terminator = function
        .operation(terminator_id)
        .ok_or(JsCodegenError::UnknownOperation {
            operation: terminator_id,
        })?;
    let Some(discriminant) = terminator.operands().first().copied() else {
        return Err(JsCodegenError::MalformedOperation {
            operation: terminator_id,
        });
    };
    let successors = terminator.successors();
    let expected_successors =
        operation.cases().len() + usize::from(operation.no_match_target().is_some());

    if successors.len() != expected_successors {
        return Err(JsCodegenError::MalformedOperation {
            operation: terminator_id,
        });
    }

    let expected_operands = 1
        + operation
            .cases()
            .iter()
            .map(|case| case.target().argument_count())
            .sum::<usize>()
        + operation
            .no_match_target()
            .map_or(0, |target| target.argument_count());

    if terminator.operands().len() != expected_operands {
        return Err(JsCodegenError::MalformedOperation {
            operation: terminator_id,
        });
    }

    let needs_matched_flag = operation
        .cases()
        .iter()
        .any(|case| case.target().argument_count() != 0)
        || operation
            .no_match_target()
            .is_some_and(|target| target.argument_count() != 0);
    let matched_flag = needs_matched_flag.then(|| locals.allocate());
    let mut nested_controls = active_controls.to_vec();

    nested_controls.push(ActiveControl {
        structure_entry: switch_block,
        produced_block: None,
        continue_target: None,
        break_target: operation.completion_block(),
        label: operation.labels().last().map(Box::as_ref),
        completion_flag: None,
    });

    let mut cases = Vec::with_capacity(operation.cases().len());

    for (index, case) in operation.cases().iter().enumerate() {
        let successor = successors[index];

        if successor.target().block() != case.target().block() {
            return Err(JsCodegenError::MalformedOperation {
                operation: terminator_id,
            });
        }

        let entry = case.target().block();
        let body = if let Some(transfer) = structured_transfer(entry, &nested_controls) {
            JsControlSequence {
                steps: vec![transfer],
            }
        } else {
            let stop = operation
                .cases()
                .get(index + 1)
                .map_or(operation.completion_block(), |next| next.target().block());

            plan_sequence(
                context,
                locals,
                entry,
                Some(stop),
                visited,
                &nested_controls,
            )?
        };

        cases.push(JsSwitchCasePlan::new(
            case.test_region(),
            JsEdgeKey::new(terminator_id, index),
            body,
        ));
    }

    let no_match_edge = if let Some(target) = operation.no_match_target() {
        let successor_index = operation.cases().len();
        let successor = successors[successor_index];

        if successor.target().block() != target.block() {
            return Err(JsCodegenError::MalformedOperation {
                operation: terminator_id,
            });
        }

        Some(JsEdgeKey::new(terminator_id, successor_index))
    } else {
        None
    };

    Ok(JsSwitchPlan::new(
        operation.labels().into(),
        discriminant,
        matched_flag,
        cases.into_boxed_slice(),
        no_match_edge,
    ))
}
