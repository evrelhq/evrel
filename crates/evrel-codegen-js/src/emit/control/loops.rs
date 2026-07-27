//! Loop and iterator emission.

use super::*;

pub(super) fn iterator_left<'ast>(
    builder: &AstBuilder<'ast>,
    function_plan: &JsFunctionPlan,
    local: JsLocalId,
) -> ForStatementLeft<'ast> {
    let name = function_plan
        .local_name(local)
        .expect("every produced iterator local must receive a name");
    let declarator = VariableDeclarator::new(
        SPAN,
        VariableDeclarationKind::Let,
        BindingPattern::new_binding_identifier(SPAN, builder.allocator().alloc_str(name), builder),
        None::<ArenaBox<'ast, TSTypeAnnotation<'ast>>>,
        None,
        false,
        builder,
    );

    ForStatementLeft::new_variable_declaration(
        SPAN,
        VariableDeclarationKind::Let,
        ArenaVec::from_array_in([declarator], builder),
        false,
        builder,
    )
}

pub(super) fn emit_control_sequence_as_expressions<'ast>(
    builder: &AstBuilder<'ast>,
    module: &ModuleIr,
    output_plan: &JsModulePlan,
    function: &FunctionIr,
    function_plan: &JsFunctionPlan,
    sequence: &JsControlSequence,
) -> Result<ArenaVec<'ast, Expression<'ast>>, JsCodegenError> {
    let mut expressions = ArenaVec::new_in(builder);

    for step in sequence.steps() {
        match step {
            JsControlStep::Block(block) => {
                let block_data = function
                    .block(*block)
                    .ok_or(JsCodegenError::UnknownBlock { block: *block })?;
                expressions.extend(emit_operations_as_expressions(
                    builder,
                    module,
                    output_plan,
                    function,
                    function_plan,
                    block_data.operations(),
                )?);
            }
            JsControlStep::Edge(edge) => expressions.extend(emit_edge_transfer_expressions(
                builder,
                function,
                function_plan,
                *edge,
            )?),
            JsControlStep::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let consequent = optional_expression_sequence(
                    builder,
                    emit_control_sequence_as_expressions(
                        builder,
                        module,
                        output_plan,
                        function,
                        function_plan,
                        then_branch,
                    )?,
                )
                .unwrap_or_else(|| {
                    Expression::new_unary_expression(
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
                    )
                });
                let alternate = optional_expression_sequence(
                    builder,
                    emit_control_sequence_as_expressions(
                        builder,
                        module,
                        output_plan,
                        function,
                        function_plan,
                        else_branch,
                    )?,
                )
                .unwrap_or_else(|| {
                    Expression::new_unary_expression(
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
                    )
                });
                expressions.push(Expression::new_conditional_expression(
                    SPAN,
                    emit_value_expression(builder, function, function_plan, *condition)?,
                    consequent,
                    alternate,
                    builder,
                ));
            }
            _ => {
                return Err(JsCodegenError::UnsupportedForHeader {
                    reason: concat!(file!(), ":", line!()),
                });
            }
        }
    }

    Ok(expressions)
}

pub(super) fn emit_for_initializer_sequence<'ast>(
    builder: &AstBuilder<'ast>,
    module: &ModuleIr,
    output_plan: &JsModulePlan,
    function: &FunctionIr,
    function_plan: &JsFunctionPlan,
    sequence: &JsControlSequence,
) -> Result<ArenaVec<'ast, Statement<'ast>>, JsCodegenError> {
    let mut statements = ArenaVec::new_in(builder);

    for step in sequence.steps() {
        match step {
            JsControlStep::Block(block) => emit_block_operations(
                builder,
                module,
                output_plan,
                function,
                function_plan,
                &mut statements,
                *block,
            )?,
            JsControlStep::Edge(edge) => {
                emit_edge_transfer(builder, function, function_plan, &mut statements, *edge)?;
            }

            JsControlStep::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let consequent = optional_expression_sequence(
                    builder,
                    emit_control_sequence_as_expressions(
                        builder,
                        module,
                        output_plan,
                        function,
                        function_plan,
                        then_branch,
                    )?,
                )
                .unwrap_or_else(|| {
                    Expression::new_unary_expression(
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
                    )
                });
                let alternate = optional_expression_sequence(
                    builder,
                    emit_control_sequence_as_expressions(
                        builder,
                        module,
                        output_plan,
                        function,
                        function_plan,
                        else_branch,
                    )?,
                )
                .unwrap_or_else(|| {
                    Expression::new_unary_expression(
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
                    )
                });
                let expression = Expression::new_conditional_expression(
                    SPAN,
                    emit_value_expression(builder, function, function_plan, *condition)?,
                    consequent,
                    alternate,
                    builder,
                );
                statements.push(Statement::new_expression_statement(
                    SPAN, expression, builder,
                ));
            }
            _ => {
                return Err(JsCodegenError::UnsupportedForHeader {
                    reason: concat!(file!(), ":", line!()),
                });
            }
        }
    }

    Ok(statements)
}

pub(super) fn emit_native_for_initializer<'ast>(
    builder: &AstBuilder<'ast>,
    statements: ArenaVec<'ast, Statement<'ast>>,
) -> Result<Option<ForStatementInit<'ast>>, JsCodegenError> {
    let mut declaration: Option<ArenaBox<'ast, oxc_ast::ast::VariableDeclaration<'ast>>> = None;
    let mut expressions = ArenaVec::new_in(builder);

    for statement in statements {
        match statement {
            Statement::VariableDeclaration(mut current) => {
                if !expressions.is_empty() {
                    let Some(first) = current.declarations.first_mut() else {
                        return Err(JsCodegenError::UnsupportedForHeader {
                            reason: concat!(file!(), ":", line!()),
                        });
                    };
                    let Some(initializer) = first.init.take() else {
                        return Err(JsCodegenError::UnsupportedForHeader {
                            reason: concat!(file!(), ":", line!()),
                        });
                    };

                    expressions.push(initializer);
                    first.init = Some(expression_sequence(
                        builder,
                        std::mem::replace(&mut expressions, ArenaVec::new_in(builder)),
                    ));
                }

                if let Some(existing) = &mut declaration {
                    if existing.kind != current.kind {
                        current.kind = existing.kind;
                        for declarator in &mut current.declarations {
                            declarator.kind = existing.kind;
                        }
                    }

                    existing.declarations.extend(current.unbox().declarations);
                } else {
                    declaration = Some(current);
                }
            }
            Statement::ExpressionStatement(statement) => {
                let expression = statement.unbox().expression;
                if let Some(existing) = &mut declaration {
                    match expression {
                        Expression::AssignmentExpression(assignment)
                            if assignment.operator == AssignmentOperator::Assign =>
                        {
                            let assignment = assignment.unbox();
                            match assignment.left {
                                AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                                    let kind = existing.kind;
                                    let initializer = if expressions.is_empty() {
                                        assignment.right
                                    } else {
                                        expressions.push(assignment.right);
                                        expression_sequence(
                                            builder,
                                            std::mem::replace(
                                                &mut expressions,
                                                ArenaVec::new_in(builder),
                                            ),
                                        )
                                    };
                                    existing.declarations.push(VariableDeclarator::new(
                                        SPAN,
                                        kind,
                                        BindingPattern::new_binding_identifier(
                                            SPAN,
                                            builder.allocator().alloc_str(identifier.name.as_str()),
                                            builder,
                                        ),
                                        None::<ArenaBox<'ast, TSTypeAnnotation<'ast>>>,
                                        Some(initializer),
                                        false,
                                        builder,
                                    ));
                                }
                                target => expressions.push(Expression::new_assignment_expression(
                                    SPAN,
                                    AssignmentOperator::Assign,
                                    target,
                                    assignment.right,
                                    builder,
                                )),
                            }
                        }
                        expression => expressions.push(expression),
                    }
                } else {
                    expressions.push(expression);
                }
            }
            _ => {
                return Err(JsCodegenError::UnsupportedForHeader {
                    reason: concat!(file!(), ":", line!()),
                });
            }
        }
    }

    if let Some(mut declaration) = declaration {
        if !expressions.is_empty() {
            let Some(last) = declaration.declarations.last_mut() else {
                return Err(JsCodegenError::UnsupportedForHeader {
                    reason: concat!(file!(), ":", line!()),
                });
            };
            let Some(initializer) = last.init.take() else {
                return Err(JsCodegenError::UnsupportedForHeader {
                    reason: concat!(file!(), ":", line!()),
                });
            };
            let mut elements = ArenaVec::with_capacity_in(expressions.len() + 1, builder);
            elements.push(ArrayExpressionElement::from(initializer));
            elements.extend(expressions.into_iter().map(ArrayExpressionElement::from));
            let values = Expression::new_array_expression(SPAN, elements, builder);
            let zero = Expression::new_numeric_literal(
                SPAN,
                0.0,
                None,
                oxc_ast::ast::NumberBase::Decimal,
                builder,
            );
            last.init = Some(Expression::from(emit_computed_member_expression(
                builder, values, zero,
            )));
        }
        Ok(Some(ForStatementInit::VariableDeclaration(declaration)))
    } else {
        Ok(optional_expression_sequence(builder, expressions).map(Into::into))
    }
}

pub(super) fn emit_block_loop_test<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    block: BlockId,
    condition: evrel_ir::ValueId,
    body_edge: JsEdgeKey,
    exit_edge: JsEdgeKey,
) -> Result<Expression<'ast>, JsCodegenError> {
    let builder = emission.builder;
    let module = emission.module;
    let output_plan = emission.output_plan;
    let function = emission.function;
    let function_plan = emission.plan;

    let block_data = function
        .block(block)
        .ok_or(JsCodegenError::UnknownBlock { block })?;
    let mut expressions = emit_operations_as_expressions(
        builder,
        module,
        output_plan,
        function,
        function_plan,
        block_data.operations(),
    )?;
    let condition = emit_value_expression(builder, function, function_plan, condition)?;
    let mut when_true =
        emit_edge_transfer_expressions(builder, function, function_plan, body_edge)?;
    let mut when_false =
        emit_edge_transfer_expressions(builder, function, function_plan, exit_edge)?;
    if when_true.is_empty() && when_false.is_empty() {
        expressions.push(condition);
    } else {
        when_true.push(Expression::new_boolean_literal(SPAN, true, builder));
        when_false.push(Expression::new_boolean_literal(SPAN, false, builder));
        expressions.push(Expression::new_conditional_expression(
            SPAN,
            condition,
            expression_sequence(builder, when_true),
            expression_sequence(builder, when_false),
            builder,
        ));
    }
    Ok(expression_sequence(builder, expressions))
}

pub(super) fn emit_native_for_test<'ast>(
    builder: &AstBuilder<'ast>,
    module: &ModuleIr,
    output_plan: &JsModulePlan,
    function: &FunctionIr,
    function_plan: &JsFunctionPlan,
    enter_test_edge: JsEdgeKey,
    test: &JsForTestPlan,
) -> Result<Option<Expression<'ast>>, JsCodegenError> {
    let mut expressions =
        emit_edge_transfer_expressions(builder, function, function_plan, enter_test_edge)?;
    let (block, body_edge) = match test {
        JsForTestPlan::Always { block, body_edge } => (*block, *body_edge),
        JsForTestPlan::Conditional {
            block,
            condition,
            body_edge,
            exit_edge,
        } => {
            expressions.push(emit_block_loop_test(
                FunctionEmission::new(builder, module, output_plan, function, function_plan),
                *block,
                *condition,
                *body_edge,
                *exit_edge,
            )?);
            return Ok(Some(expression_sequence(builder, expressions)));
        }
        JsForTestPlan::Flow {
            value_flow,
            body_edge,
            exit_edge,
        } => {
            expressions.push(emit_loop_test(LoopTestEmission {
                function: FunctionEmission::new(
                    builder,
                    module,
                    output_plan,
                    function,
                    function_plan,
                ),
                value_flow,
                body_edge: *body_edge,
                exit_edge: *exit_edge,
            })?);
            return Ok(Some(expression_sequence(builder, expressions)));
        }
    };
    let block_data = function
        .block(block)
        .ok_or(JsCodegenError::UnknownBlock { block })?;
    expressions.extend(emit_operations_as_expressions(
        builder,
        module,
        output_plan,
        function,
        function_plan,
        block_data.operations(),
    )?);
    expressions.extend(emit_edge_transfer_expressions(
        builder,
        function,
        function_plan,
        body_edge,
    )?);
    if expressions.is_empty() {
        return Ok(None);
    }
    expressions.push(Expression::new_boolean_literal(SPAN, true, builder));

    Ok(Some(expression_sequence(builder, expressions)))
}
