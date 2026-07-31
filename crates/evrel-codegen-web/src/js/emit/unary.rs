//! JavaScript unary expression emission.

use evrel_js_ir::{
    OperationId, TypeofOp, TypeofTarget, UnaryOp, UnaryOperator as IrUnaryOperator, ValueId,
};
use oxc_allocator::GetAllocator;
use oxc_ast::{
    AstBuilder,
    ast::{Expression, UnaryOperator},
};
use oxc_span::SPAN;

use crate::JsCodegenError;

use super::{FunctionEmission, value::emit_value_expression};

/// Emits one value-based JavaScript unary operation.
pub(crate) fn emit_unary_expression<'ast>(
    builder: &AstBuilder<'ast>,
    operation: &UnaryOp,
    argument: Expression<'ast>,
) -> Expression<'ast> {
    Expression::new_unary_expression(
        SPAN,
        emit_unary_operator(operation.operator()),
        argument,
        builder,
    )
}

/// Emits JavaScript `typeof` while preserving its two semantic input forms.
pub(crate) fn emit_typeof_expression<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    operation_id: OperationId,
    operation: &TypeofOp,
    operands: &[ValueId],
) -> Result<Expression<'ast>, JsCodegenError> {
    let builder = emission.builder;

    let argument = match operation.target() {
        TypeofTarget::Value => {
            let [value] = operands else {
                return Err(JsCodegenError::MalformedOperation {
                    operation: operation_id,
                });
            };

            emit_value_expression(builder, emission.function, emission.plan, *value)?
        }

        TypeofTarget::Global(name) => {
            let [] = operands else {
                return Err(JsCodegenError::MalformedOperation {
                    operation: operation_id,
                });
            };

            Expression::new_identifier(SPAN, builder.allocator().alloc_str(name), builder)
        }
    };

    Ok(Expression::new_unary_expression(
        SPAN,
        UnaryOperator::Typeof,
        argument,
        builder,
    ))
}

const fn emit_unary_operator(operator: IrUnaryOperator) -> UnaryOperator {
    match operator {
        IrUnaryOperator::Plus => UnaryOperator::UnaryPlus,
        IrUnaryOperator::Negate => UnaryOperator::UnaryNegation,

        IrUnaryOperator::BitwiseNot => UnaryOperator::BitwiseNot,

        IrUnaryOperator::LogicalNot => UnaryOperator::LogicalNot,

        IrUnaryOperator::Void => UnaryOperator::Void,
    }
}

#[cfg(test)]
mod tests {
    use evrel_js_ir::{ConstantValue, UnaryOp, UnaryOperator as IrUnaryOperator};
    use oxc_allocator::Allocator;
    use oxc_ast::AstBuilder;
    use oxc_codegen::Codegen;

    use crate::js::emit::constant::emit_constant_expression;

    use super::emit_unary_expression;

    #[test]
    fn emits_unary_expressions() {
        let allocator = Allocator::default();
        let builder = AstBuilder::new(&allocator);
        let argument = emit_constant_expression(&builder, &ConstantValue::Boolean(false));

        let expression = emit_unary_expression(
            &builder,
            &UnaryOp::new(IrUnaryOperator::LogicalNot),
            argument,
        );

        let mut codegen = Codegen::new();
        codegen.print_expression(&expression);

        assert_eq!(codegen.into_source_text(), "!false");
    }
}
