//! Validated straight-line operation-sequence emission.
//!
//! This module owns conversion of planner-approved operation sequences into
//! JavaScript expressions. Structural recognition remains in `plan`, while
//! statement and control-flow construction remains in `js::emit::control`.

use evrel_js_ir::{JsFunctionIr, JsModuleIr, OperationId};
use oxc_allocator::Vec as ArenaVec;
use oxc_ast::{
    AstBuilder,
    ast::{Expression, Statement},
};
use oxc_span::SPAN;

use crate::{
    JsCodegenError,
    js::plan::{JsFunctionPlan, JsModulePlan},
};

use super::operation::emit_operation;

pub(crate) fn emit_operations<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    output_plan: &JsModulePlan,
    function: &JsFunctionIr,
    function_plan: &JsFunctionPlan,
    statements: &mut ArenaVec<'ast, Statement<'ast>>,
    operations: &[OperationId],
) -> Result<(), JsCodegenError> {
    for &operation in operations {
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

    Ok(())
}

pub(crate) fn emit_operations_as_expressions<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    output_plan: &JsModulePlan,
    function: &JsFunctionIr,
    function_plan: &JsFunctionPlan,
    operations: &[OperationId],
) -> Result<ArenaVec<'ast, Expression<'ast>>, JsCodegenError> {
    let mut expressions = ArenaVec::new_in(builder);

    for &operation in operations {
        let mut statements = ArenaVec::new_in(builder);
        emit_operation(
            builder,
            module,
            output_plan,
            function,
            function_plan,
            &mut statements,
            operation,
        )?;

        for statement in statements {
            match statement {
                Statement::ExpressionStatement(statement) => {
                    expressions.push(statement.unbox().expression);
                }
                _ => {
                    return Err(JsCodegenError::UnsupportedOperation {
                        operation,
                        reason: concat!(file!(), ":", line!()),
                    });
                }
            }
        }
    }

    Ok(expressions)
}

pub(crate) fn optional_expression_sequence<'ast>(
    builder: &AstBuilder<'ast>,
    mut expressions: ArenaVec<'ast, Expression<'ast>>,
) -> Option<Expression<'ast>> {
    match expressions.len() {
        0 => None,
        1 => expressions.pop(),
        _ => Some(Expression::new_sequence_expression(
            SPAN,
            expressions,
            builder,
        )),
    }
}

pub(crate) fn expression_sequence<'ast>(
    builder: &AstBuilder<'ast>,
    expressions: ArenaVec<'ast, Expression<'ast>>,
) -> Expression<'ast> {
    optional_expression_sequence(builder, expressions)
        .expect("an expression sequence must contain at least one expression")
}
