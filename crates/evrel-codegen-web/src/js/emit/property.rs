//! JavaScript public-property emission.

use evrel_js_ir::{JsModuleIr, PrivateNameId};
use oxc_allocator::GetAllocator;
use oxc_ast::{
    AstBuilder,
    ast::{
        AssignmentOperator, AssignmentTarget, Expression, IdentifierName, MemberExpression,
        PrivateIdentifier, Statement,
    },
};
use oxc_span::SPAN;
use oxc_syntax::identifier::is_identifier_name;

/// Emits a property with a statically known name.
pub(crate) fn emit_static_member_expression<'ast>(
    builder: &AstBuilder<'ast>,
    object: Expression<'ast>,
    name: &str,
) -> MemberExpression<'ast> {
    let name = builder.allocator().alloc_str(name);

    if is_identifier_name(name) {
        MemberExpression::new_static_member_expression(
            SPAN,
            object,
            IdentifierName::new(SPAN, name, builder),
            false,
            builder,
        )
    } else {
        let property = Expression::new_string_literal(SPAN, name, None, builder);

        MemberExpression::new_computed_member_expression(SPAN, object, property, false, builder)
    }
}

/// Emits a property whose key is computed at runtime.
pub(crate) fn emit_computed_member_expression<'ast>(
    builder: &AstBuilder<'ast>,
    object: Expression<'ast>,
    key: Expression<'ast>,
) -> MemberExpression<'ast> {
    MemberExpression::new_computed_member_expression(SPAN, object, key, false, builder)
}

pub(crate) fn emit_private_member_expression<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    object: Expression<'ast>,
    private_name: PrivateNameId,
) -> Result<MemberExpression<'ast>, crate::JsCodegenError> {
    let private_name = module
        .private_name(private_name)
        .ok_or(crate::JsCodegenError::UnknownPrivateName { private_name })?;

    Ok(MemberExpression::new_private_field_expression(
        SPAN,
        object,
        PrivateIdentifier::new(
            SPAN,
            builder.allocator().alloc_str(private_name.name()),
            builder,
        ),
        false,
        builder,
    ))
}

/// Converts a member expression into a property-read expression.
pub(crate) fn emit_property_read_expression<'ast>(
    member: MemberExpression<'ast>,
) -> Expression<'ast> {
    Expression::from(member)
}

/// Emits a plain property assignment statement.
pub(crate) fn emit_property_store_statement<'ast>(
    builder: &AstBuilder<'ast>,
    member: MemberExpression<'ast>,
    value: Expression<'ast>,
) -> Statement<'ast> {
    let assignment = Expression::new_assignment_expression(
        SPAN,
        AssignmentOperator::Assign,
        AssignmentTarget::from(member),
        value,
        builder,
    );

    Statement::new_expression_statement(SPAN, assignment, builder)
}
