//! Shared control-flow emission primitives.

use super::*;

#[derive(Clone, Copy)]
pub(super) struct LoopTestEmission<'emit, 'ast> {
    pub(super) function: FunctionEmission<'emit, 'ast>,
    pub(super) value_flow: &'emit crate::plan::JsValueFlowPlan,
    pub(super) body_edge: JsEdgeKey,
    pub(super) exit_edge: JsEdgeKey,
}

pub(super) fn emit_loop_test<'ast>(
    emission: LoopTestEmission<'_, 'ast>,
) -> Result<Expression<'ast>, JsCodegenError> {
    let builder = emission.function.builder;
    let function = emission.function.function;
    let function_plan = emission.function.plan;
    let flow = emission.value_flow;
    let mut expressions = ArenaVec::new_in(builder);
    let condition = emit_value_flow(emission.function, flow)?;
    let mut when_true =
        emit_edge_transfer_expressions(builder, function, function_plan, emission.body_edge)?;
    when_true.push(Expression::new_boolean_literal(SPAN, true, builder));
    let mut when_false =
        emit_edge_transfer_expressions(builder, function, function_plan, emission.exit_edge)?;
    when_false.push(Expression::new_boolean_literal(SPAN, false, builder));
    expressions.push(Expression::new_conditional_expression(
        SPAN,
        condition,
        expression_sequence(builder, when_true),
        expression_sequence(builder, when_false),
        builder,
    ));
    Ok(expression_sequence(builder, expressions))
}

pub(super) fn emit_value_flow<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    flow: &crate::plan::JsValueFlowPlan,
) -> Result<Expression<'ast>, JsCodegenError> {
    let FunctionEmission {
        builder,
        module,
        output_plan,
        function,
        plan,
    } = emission;
    let mut expressions = ArenaVec::new_in(builder);
    expressions.push(emit_value_flow_step(emission, flow.root())?);
    let completion = function
        .block(flow.result_block())
        .ok_or(JsCodegenError::UnknownBlock {
            block: flow.result_block(),
        })?;
    expressions.extend(emit_operations_as_expressions(
        builder,
        module,
        output_plan,
        function,
        plan,
        completion.operations(),
    )?);
    expressions.push(emit_value_expression(
        builder,
        function,
        plan,
        flow.result(),
    )?);

    Ok(expression_sequence(builder, expressions))
}

pub(super) fn emit_value_flow_step<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    step: &JsValueFlowStep,
) -> Result<Expression<'ast>, JsCodegenError> {
    let FunctionEmission {
        builder,
        module,
        output_plan,
        function,
        plan: function_plan,
    } = emission;
    let JsValueFlowStep::Block {
        block,
        continuation,
    } = step
    else {
        return Ok(Expression::new_unary_expression(
            SPAN,
            oxc_syntax::operator::UnaryOperator::Void,
            Expression::new_numeric_literal(
                SPAN,
                0.0,
                None,
                oxc_ast::ast::NumberBase::Decimal,
                builder,
            ),
            builder,
        ));
    };
    let block_data = function
        .block(*block)
        .ok_or(JsCodegenError::UnknownBlock { block: *block })?;
    let mut expressions = emit_operations_as_expressions(
        builder,
        module,
        output_plan,
        function,
        function_plan,
        block_data.operations(),
    )?;
    let emit_branch = |edge: JsEdgeKey, next: &JsValueFlowStep| {
        let mut branch = emit_edge_transfer_expressions(builder, function, function_plan, edge)?;
        branch.push(emit_value_flow_step(emission, next)?);
        Ok(expression_sequence(builder, branch))
    };
    let continuation = match continuation {
        JsValueFlowContinuation::Jump { edge, next } => emit_branch(*edge, next)?,
        JsValueFlowContinuation::Branch {
            condition,
            then_edge,
            then_step,
            else_edge,
            else_step,
        } => {
            let consequent = emit_branch(*then_edge, then_step)?;
            let alternate = emit_branch(*else_edge, else_step)?;
            Expression::new_conditional_expression(
                SPAN,
                emit_value_expression(builder, function, function_plan, *condition)?,
                consequent,
                alternate,
                builder,
            )
        }
    };
    expressions.push(continuation);
    Ok(expression_sequence(builder, expressions))
}

pub(super) fn local_assignment_statement<'ast>(
    builder: &AstBuilder<'ast>,
    name: &str,
    value: Expression<'ast>,
) -> Statement<'ast> {
    let assignment = Expression::new_assignment_expression(
        SPAN,
        AssignmentOperator::Assign,
        AssignmentTarget::new_assignment_target_identifier(
            SPAN,
            builder.allocator().alloc_str(name),
            builder,
        ),
        value,
        builder,
    );

    Statement::new_expression_statement(SPAN, assignment, builder)
}

pub(super) fn wrap_labels<'ast>(
    builder: &AstBuilder<'ast>,
    labels: &[Box<str>],
    mut statement: Statement<'ast>,
) -> Statement<'ast> {
    for label in labels.iter().rev() {
        statement = Statement::new_labeled_statement(
            SPAN,
            LabelIdentifier::new(SPAN, builder.allocator().alloc_str(label), builder),
            statement,
            builder,
        );
    }

    statement
}

pub(super) fn emit_hoisted_local_declaration<'ast>(
    builder: &AstBuilder<'ast>,
    plan: &JsFunctionPlan,
) -> Result<Option<Statement<'ast>>, JsCodegenError> {
    if plan.local_count() == 0 {
        return Ok(None);
    }

    let mut declarators = ArenaVec::with_capacity_in(plan.local_count(), builder);

    for index in 0..plan.local_count() {
        let local = JsLocalId::from_index(index);
        let name = plan
            .local_name(local)
            .expect("every planned local must receive a name");
        declarators.push(VariableDeclarator::new(
            SPAN,
            VariableDeclarationKind::Let,
            BindingPattern::new_binding_identifier(
                SPAN,
                builder.allocator().alloc_str(name),
                builder,
            ),
            None::<ArenaBox<'ast, TSTypeAnnotation<'ast>>>,
            None,
            false,
            builder,
        ));
    }

    Ok(Some(Statement::new_variable_declaration(
        SPAN,
        VariableDeclarationKind::Let,
        declarators,
        false,
        builder,
    )))
}

pub(super) fn emit_edge_transfer<'ast>(
    builder: &AstBuilder<'ast>,
    function: &FunctionIr,
    plan: &JsFunctionPlan,
    statements: &mut ArenaVec<'ast, Statement<'ast>>,
    edge: JsEdgeKey,
) -> Result<(), JsCodegenError> {
    let expressions = emit_edge_transfer_expressions(builder, function, plan, edge)?;

    for expression in expressions {
        statements.push(Statement::new_expression_statement(
            SPAN, expression, builder,
        ));
    }

    Ok(())
}

pub(crate) fn emit_edge_transfer_expressions<'ast>(
    builder: &AstBuilder<'ast>,
    function: &FunctionIr,
    plan: &JsFunctionPlan,
    edge: JsEdgeKey,
) -> Result<ArenaVec<'ast, Expression<'ast>>, JsCodegenError> {
    let transfer = plan
        .edge_transfer(edge)
        .ok_or(JsCodegenError::MalformedOperation {
            operation: edge.terminator(),
        })?;

    if transfer.is_empty() {
        return Ok(ArenaVec::new_in(builder));
    }

    let mut expressions = ArenaVec::with_capacity_in(transfer.moves().len(), builder);

    for movement in transfer.moves() {
        let destination =
            plan.local_name(movement.destination())
                .ok_or(JsCodegenError::MalformedOperation {
                    operation: edge.terminator(),
                })?;
        let source = match movement.source() {
            JsMoveSource::Binding(binding) => Expression::new_identifier(
                SPAN,
                builder.allocator().alloc_str(binding_name(plan, binding)?),
                builder,
            ),
            JsMoveSource::Local(local) => {
                let source = plan
                    .local_name(local)
                    .ok_or(JsCodegenError::MalformedOperation {
                        operation: edge.terminator(),
                    })?;

                Expression::new_identifier(SPAN, builder.allocator().alloc_str(source), builder)
            }
            JsMoveSource::Inline(value) => emit_value_expression(builder, function, plan, value)?,
        };
        let assignment = Expression::new_assignment_expression(
            SPAN,
            AssignmentOperator::Assign,
            AssignmentTarget::new_assignment_target_identifier(
                SPAN,
                builder.allocator().alloc_str(destination),
                builder,
            ),
            source,
            builder,
        );

        expressions.push(assignment);
    }

    Ok(expressions)
}

pub(super) fn emit_block_operations<'ast>(
    builder: &AstBuilder<'ast>,
    module: &ModuleIr,
    output_plan: &JsModulePlan,
    function: &FunctionIr,
    function_plan: &JsFunctionPlan,
    statements: &mut ArenaVec<'ast, Statement<'ast>>,
    block: BlockId,
) -> Result<(), JsCodegenError> {
    let block_data = function
        .block(block)
        .ok_or(JsCodegenError::UnknownBlock { block })?;

    for &operation in block_data.operations() {
        emit_operation(
            builder,
            module,
            output_plan,
            function,
            function_plan,
            statements,
            operation,
        )?;
    }

    let Some(terminator) = block_data.terminator() else {
        return Ok(());
    };
    let terminator_data =
        function
            .operation(terminator)
            .ok_or(JsCodegenError::UnknownOperation {
                operation: terminator,
            })?;

    match terminator_data.kind() {
        OperationKind::Jump(_)
        | OperationKind::If(_)
        | OperationKind::While(_)
        | OperationKind::DoWhile(_)
        | OperationKind::For(_)
        | OperationKind::ForIn(_)
        | OperationKind::ForOf(_)
        | OperationKind::Switch(_)
        | OperationKind::Try(_) => Ok(()),

        OperationKind::Return(_)
            if matches!(
                function.kind(),
                evrel_ir::FunctionKind::Module | evrel_ir::FunctionKind::ClassStaticBlock
            ) =>
        {
            Ok(())
        }

        OperationKind::Return(_) | OperationKind::Throw(_) => emit_operation(
            builder,
            module,
            output_plan,
            function,
            function_plan,
            statements,
            terminator,
        ),

        _ => Err(JsCodegenError::UnsupportedOperation {
            operation: terminator,
            reason: concat!(file!(), ":", line!()),
        }),
    }
}
