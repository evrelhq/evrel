//! Simple JavaScript binding emission.

use evrel_js_ir::{BindingId, BindingKind, JsModuleIr, OperationId};
use oxc_allocator::{Box as ArenaBox, GetAllocator, Vec as ArenaVec};
use oxc_ast::{
    AstBuilder,
    ast::{
        AssignmentOperator, AssignmentTarget, BindingPattern, Expression, Statement,
        TSTypeAnnotation, VariableDeclarationKind, VariableDeclarator,
    },
};
use oxc_span::SPAN;

use crate::{JsCodegenError, js::plan::JsFunctionPlan};

pub(crate) fn binding_name(
    plan: &JsFunctionPlan,
    binding: BindingId,
) -> Result<&str, JsCodegenError> {
    plan.binding_name(binding)
        .ok_or(JsCodegenError::UnknownBinding { binding })
}

/// Emits the first runtime initialization of a simple binding.
pub(crate) fn emit_initialize_binding_statement<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    plan: &JsFunctionPlan,
    operation: OperationId,
    binding: BindingId,
    value: Option<Expression<'ast>>,
) -> Result<Statement<'ast>, JsCodegenError> {
    let binding_data = module
        .binding(binding)
        .ok_or(JsCodegenError::UnknownBinding { binding })?;

    let kind = match binding_data.kind() {
        BindingKind::Const => VariableDeclarationKind::Const,
        BindingKind::Let | BindingKind::Class | BindingKind::Catch => VariableDeclarationKind::Let,
        BindingKind::Var | BindingKind::Function => VariableDeclarationKind::Var,

        BindingKind::Import | BindingKind::Parameter => {
            return Err(JsCodegenError::UnsupportedOperation {
                operation,
                reason: concat!(file!(), ":", line!()),
            });
        }
    };

    let name = binding_name(plan, binding)?;
    let declarator = VariableDeclarator::new(
        SPAN,
        kind,
        BindingPattern::new_binding_identifier(SPAN, builder.allocator().alloc_str(name), builder),
        None::<ArenaBox<'ast, TSTypeAnnotation<'ast>>>,
        value,
        false,
        builder,
    );

    Ok(Statement::new_variable_declaration(
        SPAN,
        kind,
        ArenaVec::from_array_in([declarator], builder),
        false,
        builder,
    ))
}

pub(crate) fn emit_load_binding_expression<'ast>(
    builder: &AstBuilder<'ast>,
    plan: &JsFunctionPlan,
    binding: BindingId,
) -> Result<Expression<'ast>, JsCodegenError> {
    let name = binding_name(plan, binding)?;

    Ok(Expression::new_identifier(
        SPAN,
        builder.allocator().alloc_str(name),
        builder,
    ))
}

pub(crate) fn emit_store_binding_statement<'ast>(
    builder: &AstBuilder<'ast>,
    plan: &JsFunctionPlan,
    binding: BindingId,
    value: Expression<'ast>,
) -> Result<Statement<'ast>, JsCodegenError> {
    let assignment = emit_store_binding_expression(builder, plan, binding, value)?;

    Ok(Statement::new_expression_statement(
        SPAN, assignment, builder,
    ))
}

pub(crate) fn emit_store_binding_expression<'ast>(
    builder: &AstBuilder<'ast>,
    plan: &JsFunctionPlan,
    binding: BindingId,
    value: Expression<'ast>,
) -> Result<Expression<'ast>, JsCodegenError> {
    let name = builder.allocator().alloc_str(binding_name(plan, binding)?);
    Ok(Expression::new_assignment_expression(
        SPAN,
        AssignmentOperator::Assign,
        AssignmentTarget::new_assignment_target_identifier(SPAN, name, builder),
        value,
        builder,
    ))
}
