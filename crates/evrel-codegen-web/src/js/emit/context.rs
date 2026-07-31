//! JavaScript execution-context expression emission.

use evrel_js_ir::{MetaPropertyKind, MetaPropertyOp};
use oxc_allocator::GetAllocator;
use oxc_ast::{
    AstBuilder,
    ast::{Expression, IdentifierName},
};
use oxc_span::SPAN;

/// Emits the current JavaScript receiver value.
pub(crate) fn emit_load_this_expression<'ast>(builder: &AstBuilder<'ast>) -> Expression<'ast> {
    Expression::new_this_expression(SPAN, builder)
}

/// Emits the implicit `arguments` binding.
pub(crate) fn emit_load_arguments_expression<'ast>(builder: &AstBuilder<'ast>) -> Expression<'ast> {
    Expression::new_identifier(SPAN, builder.allocator().alloc_str("arguments"), builder)
}

/// Emits an execution-context meta-property.
pub(crate) fn emit_meta_property_expression<'ast>(
    builder: &AstBuilder<'ast>,
    operation: &MetaPropertyOp,
) -> Expression<'ast> {
    let (meta, property) = match operation.kind() {
        MetaPropertyKind::ImportMeta => ("import", "meta"),
        MetaPropertyKind::NewTarget => ("new", "target"),
    };

    Expression::new_meta_property(
        SPAN,
        IdentifierName::new(SPAN, builder.allocator().alloc_str(meta), builder),
        IdentifierName::new(SPAN, builder.allocator().alloc_str(property), builder),
        builder,
    )
}
