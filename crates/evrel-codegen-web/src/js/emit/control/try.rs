//! Try statement emission.

use super::*;

pub(super) fn emit_try<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    plan: &crate::js::plan::JsTryPlan,
) -> Result<Statement<'ast>, JsCodegenError> {
    let FunctionEmission {
        builder,
        module,
        output_plan,
        function,
        plan: function_plan,
    } = emission;
    let try_body = BlockStatement::boxed(
        SPAN,
        emit_control_sequence(
            builder,
            module,
            output_plan,
            function,
            function_plan,
            plan.try_body(),
        )?,
        builder,
    );
    let catch = plan
        .catch()
        .map(|catch| {
            let Some(JsValueRepresentation::Temporary(local)) =
                function_plan.value(catch.exception())
            else {
                return Err(JsCodegenError::UnsupportedValue {
                    value: catch.exception(),
                });
            };
            let name = function_plan
                .local_name(local)
                .expect("every catch parameter local must receive a name");
            let parameter = CatchParameter::new(
                SPAN,
                BindingPattern::new_binding_identifier(
                    SPAN,
                    builder.allocator().alloc_str(name),
                    builder,
                ),
                None::<ArenaBox<'ast, TSTypeAnnotation<'ast>>>,
                builder,
            );
            let body = BlockStatement::boxed(
                SPAN,
                emit_control_sequence(
                    builder,
                    module,
                    output_plan,
                    function,
                    function_plan,
                    catch.body(),
                )?,
                builder,
            );

            Ok(CatchClause::boxed(SPAN, Some(parameter), body, builder))
        })
        .transpose()?;
    let finally = plan
        .finally()
        .map(|finally| {
            Ok(BlockStatement::boxed(
                SPAN,
                emit_control_sequence(
                    builder,
                    module,
                    output_plan,
                    function,
                    function_plan,
                    finally,
                )?,
                builder,
            ))
        })
        .transpose()?;

    Ok(Statement::new_try_statement(
        SPAN, try_body, catch, finally, builder,
    ))
}
