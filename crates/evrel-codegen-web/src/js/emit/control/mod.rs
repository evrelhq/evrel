//! Structured control-flow emission from a validated plan.

mod loops;
mod support;
mod switch;
mod r#try;
use evrel_js_ir::{BlockId, JsFunctionIr, JsModuleIr, OperationKind};
use loops::*;
use oxc_allocator::{Box as ArenaBox, GetAllocator, Vec as ArenaVec};
use oxc_ast::{
    AstBuilder,
    ast::{
        ArrayExpressionElement, AssignmentOperator, AssignmentTarget, BindingPattern,
        BlockStatement, CatchClause, CatchParameter, Expression, ForStatementInit,
        ForStatementLeft, LabelIdentifier, Statement, SwitchCase, TSTypeAnnotation,
        VariableDeclarationKind, VariableDeclarator,
    },
};
use oxc_span::SPAN;
pub(crate) use support::emit_edge_transfer_expressions;
use support::*;
use switch::*;
use r#try::*;

use crate::{
    JsCodegenError,
    js::plan::{
        JsControlSequence, JsControlStep, JsEdgeKey, JsForTestPlan, JsFunctionPlan, JsIteratorKind,
        JsLocalId, JsModulePlan, JsMoveSource, JsValueFlowContinuation, JsValueFlowStep,
        JsValueRepresentation,
    },
};

use super::{
    FunctionEmission,
    binding::binding_name,
    operation::emit_operation,
    property::emit_computed_member_expression,
    region::emit_expression_region,
    sequence::{emit_operations_as_expressions, expression_sequence, optional_expression_sequence},
    value::emit_value_expression,
};

pub(crate) fn emit_control_body<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    output_plan: &JsModulePlan,
    function: &JsFunctionIr,
    function_plan: &JsFunctionPlan,
) -> Result<ArenaVec<'ast, Statement<'ast>>, JsCodegenError> {
    let mut statements = ArenaVec::new_in(builder);

    if let Some(declaration) = emit_hoisted_local_declaration(builder, function_plan)? {
        statements.push(declaration);
    }

    let body = emit_control_sequence(
        builder,
        module,
        output_plan,
        function,
        function_plan,
        function_plan.control().body(),
    )?;
    statements.extend(body);

    Ok(statements)
}

fn emit_control_sequence<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    output_plan: &JsModulePlan,
    function: &JsFunctionIr,
    function_plan: &JsFunctionPlan,
    sequence: &JsControlSequence,
) -> Result<ArenaVec<'ast, Statement<'ast>>, JsCodegenError> {
    let mut statements = ArenaVec::new_in(builder);

    for step in sequence.steps() {
        match step {
            JsControlStep::Block(block) => {
                emit_block_operations(
                    builder,
                    module,
                    output_plan,
                    function,
                    function_plan,
                    &mut statements,
                    *block,
                )?;
            }

            JsControlStep::Edge(edge) => {
                emit_edge_transfer(builder, function, function_plan, &mut statements, *edge)?;
            }

            JsControlStep::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition =
                    emit_value_expression(builder, function, function_plan, *condition)?;
                let then_statements = emit_control_sequence(
                    builder,
                    module,
                    output_plan,
                    function,
                    function_plan,
                    then_branch,
                )?;
                let else_statements = emit_control_sequence(
                    builder,
                    module,
                    output_plan,
                    function,
                    function_plan,
                    else_branch,
                )?;
                let alternate = if else_statements.is_empty() {
                    None
                } else {
                    Some(Statement::new_block_statement(
                        SPAN,
                        else_statements,
                        builder,
                    ))
                };

                statements.push(Statement::new_if_statement(
                    SPAN,
                    condition,
                    Statement::new_block_statement(SPAN, then_statements, builder),
                    alternate,
                    builder,
                ));
            }

            JsControlStep::While {
                labels,
                test_block,
                condition,
                body_edge,
                body,
                exit_edge,
            } => {
                let condition = emit_block_loop_test(
                    FunctionEmission::new(builder, module, output_plan, function, function_plan),
                    *test_block,
                    *condition,
                    *body_edge,
                    *exit_edge,
                )?;
                let body_statements = emit_control_sequence(
                    builder,
                    module,
                    output_plan,
                    function,
                    function_plan,
                    body,
                )?;
                let loop_statement = Statement::new_while_statement(
                    SPAN,
                    condition,
                    Statement::new_block_statement(SPAN, body_statements, builder),
                    builder,
                );

                statements.push(wrap_labels(builder, labels, loop_statement));
            }

            JsControlStep::WhileFlow { labels, test, body } => {
                let condition = emit_loop_test(LoopTestEmission {
                    function: FunctionEmission::new(
                        builder,
                        module,
                        output_plan,
                        function,
                        function_plan,
                    ),
                    value_flow: test.value_flow(),
                    body_edge: test.body_edge(),
                    exit_edge: test.exit_edge(),
                })?;
                let body_statements = emit_control_sequence(
                    builder,
                    module,
                    output_plan,
                    function,
                    function_plan,
                    body,
                )?;
                let loop_statement = Statement::new_while_statement(
                    SPAN,
                    condition,
                    Statement::new_block_statement(SPAN, body_statements, builder),
                    builder,
                );

                statements.push(wrap_labels(builder, labels, loop_statement));
            }

            JsControlStep::DoWhile { labels, body, test } => {
                let loop_statements = emit_control_sequence(
                    builder,
                    module,
                    output_plan,
                    function,
                    function_plan,
                    body,
                )?;
                let condition = emit_loop_test(LoopTestEmission {
                    function: FunctionEmission::new(
                        builder,
                        module,
                        output_plan,
                        function,
                        function_plan,
                    ),
                    value_flow: test.value_flow(),
                    body_edge: test.body_edge(),
                    exit_edge: test.exit_edge(),
                })?;
                let loop_statement = Statement::new_do_while_statement(
                    SPAN,
                    Statement::new_block_statement(SPAN, loop_statements, builder),
                    condition,
                    builder,
                );

                statements.push(wrap_labels(builder, labels, loop_statement));
            }

            JsControlStep::Break {
                label,
                completion_flag,
            } => {
                if let Some(completion_flag) = completion_flag {
                    let name = function_plan
                        .local_name(*completion_flag)
                        .expect("every planned completion flag must receive a name");
                    let assignment = Expression::new_assignment_expression(
                        SPAN,
                        AssignmentOperator::Assign,
                        AssignmentTarget::new_assignment_target_identifier(
                            SPAN,
                            builder.allocator().alloc_str(name),
                            builder,
                        ),
                        Expression::new_boolean_literal(SPAN, false, builder),
                        builder,
                    );

                    statements.push(Statement::new_expression_statement(
                        SPAN, assignment, builder,
                    ));
                }

                let label = label.as_deref().map(|label| {
                    LabelIdentifier::new(SPAN, builder.allocator().alloc_str(label), builder)
                });

                statements.push(Statement::new_break_statement(SPAN, label, builder));
            }

            JsControlStep::Continue { label } => {
                let label = label.as_deref().map(|label| {
                    LabelIdentifier::new(SPAN, builder.allocator().alloc_str(label), builder)
                });

                statements.push(Statement::new_continue_statement(SPAN, label, builder));
            }

            JsControlStep::Labeled { labels, body } => {
                let body = emit_control_sequence(
                    builder,
                    module,
                    output_plan,
                    function,
                    function_plan,
                    body,
                )?;
                let statement = Statement::new_block_statement(SPAN, body, builder);

                statements.push(wrap_labels(builder, labels, statement));
            }

            JsControlStep::For(plan) => {
                let initializer = if plan.initializer_is_prelude() {
                    statements.extend(emit_control_sequence(
                        builder,
                        module,
                        output_plan,
                        function,
                        function_plan,
                        plan.initializer(),
                    )?);
                    None
                } else {
                    emit_native_for_initializer(
                        builder,
                        emit_for_initializer_sequence(
                            builder,
                            module,
                            output_plan,
                            function,
                            function_plan,
                            plan.initializer(),
                        )?,
                    )?
                };
                let test = emit_native_for_test(
                    builder,
                    module,
                    output_plan,
                    function,
                    function_plan,
                    plan.enter_test_edge(),
                    plan.test(),
                )?;
                let mut update = emit_control_sequence_as_expressions(
                    builder,
                    module,
                    output_plan,
                    function,
                    function_plan,
                    plan.update(),
                )?;
                update.extend(emit_edge_transfer_expressions(
                    builder,
                    function,
                    function_plan,
                    plan.enter_test_edge(),
                )?);
                let update = optional_expression_sequence(builder, update);
                let body = emit_control_sequence(
                    builder,
                    module,
                    output_plan,
                    function,
                    function_plan,
                    plan.body(),
                )?;
                let loop_statement = Statement::new_for_statement(
                    SPAN,
                    initializer,
                    test,
                    update,
                    Statement::new_block_statement(SPAN, body, builder),
                    builder,
                );

                statements.push(wrap_labels(builder, plan.labels(), loop_statement));
            }

            JsControlStep::Iterator(plan) => {
                let completion_name = function_plan
                    .local_name(plan.completion_flag())
                    .expect("every completion flag must receive a name");
                statements.push(local_assignment_statement(
                    builder,
                    completion_name,
                    Expression::new_boolean_literal(SPAN, true, builder),
                ));

                let iterated =
                    emit_value_expression(builder, function, function_plan, plan.iterated_value())?;
                let body = emit_control_sequence(
                    builder,
                    module,
                    output_plan,
                    function,
                    function_plan,
                    plan.body(),
                )?;
                let left = iterator_left(builder, function_plan, plan.produced_local());
                let loop_statement = match plan.kind() {
                    JsIteratorKind::In => Statement::new_for_in_statement(
                        SPAN,
                        left,
                        iterated,
                        Statement::new_block_statement(SPAN, body, builder),
                        builder,
                    ),
                    JsIteratorKind::Of | JsIteratorKind::AwaitOf => {
                        Statement::new_for_of_statement(
                            SPAN,
                            plan.kind() == JsIteratorKind::AwaitOf,
                            left,
                            iterated,
                            Statement::new_block_statement(SPAN, body, builder),
                            builder,
                        )
                    }
                };

                statements.push(wrap_labels(builder, plan.labels(), loop_statement));

                let mut natural_exit = ArenaVec::new_in(builder);
                emit_edge_transfer(
                    builder,
                    function,
                    function_plan,
                    &mut natural_exit,
                    plan.natural_exit_edge(),
                )?;

                if !natural_exit.is_empty() {
                    statements.push(Statement::new_if_statement(
                        SPAN,
                        Expression::new_identifier(
                            SPAN,
                            builder.allocator().alloc_str(completion_name),
                            builder,
                        ),
                        Statement::new_block_statement(SPAN, natural_exit, builder),
                        None,
                        builder,
                    ));
                }
            }

            JsControlStep::Switch(plan) => emit_switch(
                FunctionEmission::new(builder, module, output_plan, function, function_plan),
                plan,
                &mut statements,
            )?,

            JsControlStep::Try(plan) => statements.push(emit_try(
                FunctionEmission::new(builder, module, output_plan, function, function_plan),
                plan,
            )?),
        }
    }

    Ok(statements)
}
