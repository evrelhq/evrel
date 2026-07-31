//! JavaScript delete-expression emission.

use oxc_allocator::Vec as ArenaVec;
use oxc_ast::{
    AstBuilder,
    ast::{Expression, MemberExpression, UnaryOperator},
};
use oxc_span::SPAN;

pub(crate) fn emit_delete_value_expression<'ast>(
    builder: &AstBuilder<'ast>,
    value: Expression<'ast>,
) -> Expression<'ast> {
    Expression::new_sequence_expression(
        SPAN,
        ArenaVec::from_array_in(
            [value, Expression::new_boolean_literal(SPAN, true, builder)],
            builder,
        ),
        builder,
    )
}

pub(crate) fn emit_delete_property_expression<'ast>(
    builder: &AstBuilder<'ast>,
    member: MemberExpression<'ast>,
) -> Expression<'ast> {
    Expression::new_unary_expression(
        SPAN,
        UnaryOperator::Delete,
        Expression::from(member),
        builder,
    )
}
