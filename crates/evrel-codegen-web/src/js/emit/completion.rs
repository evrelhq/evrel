//! Direct JavaScript function-completion statement emission.

use oxc_ast::{
    AstBuilder,
    ast::{Expression, Statement},
};
use oxc_span::SPAN;

/// Emits a value-return statement.
pub(crate) fn emit_return_statement<'ast>(
    builder: &AstBuilder<'ast>,
    value: Expression<'ast>,
) -> Statement<'ast> {
    Statement::new_return_statement(SPAN, Some(value), builder)
}

/// Emits a throw statement.
pub(crate) fn emit_throw_statement<'ast>(
    builder: &AstBuilder<'ast>,
    value: Expression<'ast>,
) -> Statement<'ast> {
    Statement::new_throw_statement(SPAN, value, builder)
}
