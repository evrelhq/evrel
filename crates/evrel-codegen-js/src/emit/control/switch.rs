//! Switch statement emission.

use super::*;

pub(super) fn emit_switch<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    plan: &crate::plan::JsSwitchPlan,
    statements: &mut ArenaVec<'ast, Statement<'ast>>,
) -> Result<(), JsCodegenError> {
    let FunctionEmission {
        builder,
        module,
        output_plan,
        function,
        plan: function_plan,
    } = emission;
    let matched_name = plan.matched_flag().map(|local| {
        function_plan
            .local_name(local)
            .expect("every switch matched flag must receive a name")
    });

    if let Some(matched_name) = matched_name {
        statements.push(local_assignment_statement(
            builder,
            matched_name,
            Expression::new_boolean_literal(SPAN, false, builder),
        ));
    }

    let mut cases = ArenaVec::with_capacity_in(plan.cases().len(), builder);

    for case in plan.cases() {
        let mut consequent = ArenaVec::new_in(builder);
        let mut direct_entry = ArenaVec::new_in(builder);

        emit_edge_transfer(
            builder,
            function,
            function_plan,
            &mut direct_entry,
            case.entry_edge(),
        )?;

        if let Some(matched_name) = matched_name {
            direct_entry.push(local_assignment_statement(
                builder,
                matched_name,
                Expression::new_boolean_literal(SPAN, true, builder),
            ));
            consequent.push(Statement::new_if_statement(
                SPAN,
                Expression::new_unary_expression(
                    SPAN,
                    oxc_syntax::operator::UnaryOperator::LogicalNot,
                    Expression::new_identifier(
                        SPAN,
                        builder.allocator().alloc_str(matched_name),
                        builder,
                    ),
                    builder,
                ),
                Statement::new_block_statement(SPAN, direct_entry, builder),
                None,
                builder,
            ));
        } else {
            debug_assert!(direct_entry.is_empty());
        }

        consequent.extend(emit_control_sequence(
            builder,
            module,
            output_plan,
            function,
            function_plan,
            case.body(),
        )?);
        let test = case
            .test_region()
            .map(|region| {
                emit_expression_region(
                    builder,
                    module,
                    output_plan,
                    function,
                    function_plan,
                    region,
                )
            })
            .transpose()?;

        cases.push(SwitchCase::new(SPAN, test, consequent, builder));
    }

    let statement = Statement::new_switch_statement(
        SPAN,
        emit_value_expression(builder, function, function_plan, plan.discriminant())?,
        cases,
        builder,
    );
    statements.push(wrap_labels(builder, plan.labels(), statement));

    if let Some(no_match_edge) = plan.no_match_edge() {
        let mut transfer = ArenaVec::new_in(builder);
        emit_edge_transfer(
            builder,
            function,
            function_plan,
            &mut transfer,
            no_match_edge,
        )?;

        if !transfer.is_empty() {
            let matched_name =
                matched_name.expect("a nonempty no-match transfer requires a matched flag");
            statements.push(Statement::new_if_statement(
                SPAN,
                Expression::new_unary_expression(
                    SPAN,
                    oxc_syntax::operator::UnaryOperator::LogicalNot,
                    Expression::new_identifier(
                        SPAN,
                        builder.allocator().alloc_str(matched_name),
                        builder,
                    ),
                    builder,
                ),
                Statement::new_block_statement(SPAN, transfer, builder),
                None,
                builder,
            ));
        }
    }
    Ok(())
}
