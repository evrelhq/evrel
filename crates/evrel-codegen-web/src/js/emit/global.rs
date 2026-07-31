//! Global expression emission.

use evrel_js_ir::{LoadGlobalOp, StoreGlobalOp};
use oxc_allocator::GetAllocator;
use oxc_ast::{
    AstBuilder,
    ast::{AssignmentOperator, AssignmentTarget, Expression},
};
use oxc_span::SPAN;

/// Emits a read of an unresolved JavaScript identifier.
pub(crate) fn emit_load_global_expression<'ast>(
    builder: &AstBuilder<'ast>,
    operation: &LoadGlobalOp,
) -> Expression<'ast> {
    Expression::new_identifier(
        SPAN,
        builder.allocator().alloc_str(operation.name()),
        builder,
    )
}

/// Emits an assignment to an unresolved JavaScript identifier.
pub(crate) fn emit_store_global_expression<'ast>(
    builder: &AstBuilder<'ast>,
    operation: &StoreGlobalOp,
    value: Expression<'ast>,
) -> Expression<'ast> {
    let name = builder.allocator().alloc_str(operation.name());

    Expression::new_assignment_expression(
        SPAN,
        AssignmentOperator::Assign,
        AssignmentTarget::new_assignment_target_identifier(SPAN, name, builder),
        value,
        builder,
    )
}

#[cfg(test)]
mod tests {
    use evrel_js_ir::LoadGlobalOp;
    use oxc_allocator::Allocator;
    use oxc_ast::AstBuilder;
    use oxc_codegen::Codegen;

    use super::emit_load_global_expression;

    #[test]
    fn emits_an_unresolved_identifier_read() {
        let allocator = Allocator::default();
        let builder = AstBuilder::new(&allocator);
        let operation = LoadGlobalOp::new("console");
        let expression = emit_load_global_expression(&builder, &operation);
        let mut codegen = Codegen::new();

        codegen.print_expression(&expression);

        assert_eq!(codegen.into_source_text(), "console");
    }
}
