//! Direct JavaScript suspension-expression emission.

use evrel_js_ir::{YieldKind, YieldOp};
use oxc_ast::{AstBuilder, ast::Expression};
use oxc_span::SPAN;

/// Emits an await expression.
pub(crate) fn emit_await_expression<'ast>(
    builder: &AstBuilder<'ast>,
    value: Expression<'ast>,
) -> Expression<'ast> {
    Expression::new_await_expression(SPAN, value, builder)
}

/// Emits a yield or delegated-yield expression.
pub(crate) fn emit_yield_expression<'ast>(
    builder: &AstBuilder<'ast>,
    operation: &YieldOp,
    value: Expression<'ast>,
) -> Expression<'ast> {
    let delegate = matches!(operation.kind(), YieldKind::Delegate);

    Expression::new_yield_expression(SPAN, delegate, Some(value), builder)
}
