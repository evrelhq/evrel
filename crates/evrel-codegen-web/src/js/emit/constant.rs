//! Constant expression emission.

use evrel_js_ir::ConstantValue;
use oxc_allocator::GetAllocator;
use oxc_ast::{
    AstBuilder,
    ast::{BigintBase, BinaryOperator, Expression, NumberBase, UnaryOperator},
};
use oxc_span::SPAN;

/// Emits one constant as an Oxc expression.
pub(crate) fn emit_constant_expression<'ast>(
    builder: &AstBuilder<'ast>,
    value: &ConstantValue,
) -> Expression<'ast> {
    match value {
        ConstantValue::Undefined => Expression::new_unary_expression(
            SPAN,
            UnaryOperator::Void,
            numeric_literal(builder, 0.0),
            builder,
        ),

        ConstantValue::Boolean(value) => Expression::new_boolean_literal(SPAN, *value, builder),

        ConstantValue::Null => Expression::new_null_literal(SPAN, builder),

        ConstantValue::Number(value) => emit_number(builder, *value),

        ConstantValue::BigInt(value) => Expression::new_big_int_literal(
            SPAN,
            builder.allocator().alloc_str(value),
            None,
            BigintBase::Decimal,
            builder,
        ),

        ConstantValue::String(value) => Expression::new_string_literal_with_lone_surrogates(
            SPAN,
            builder.allocator().alloc_str(value.as_str()),
            None,
            value.has_lone_surrogates(),
            builder,
        ),
    }
}

/// Emits a JavaScript number without relying on shadowable global names.
///
/// JavaScript has no literal spellings for NaN or infinity. Expressions such
/// as `Number.NaN` and `Infinity` would be observable global/property reads, so
/// the backend emits those values using division.
fn emit_number<'ast>(builder: &AstBuilder<'ast>, value: f64) -> Expression<'ast> {
    if value.is_nan() {
        division(builder, 0.0, 0.0)
    } else if value == f64::INFINITY {
        division(builder, 1.0, 0.0)
    } else if value == f64::NEG_INFINITY {
        division(builder, -1.0, 0.0)
    } else {
        numeric_literal(builder, value)
    }
}

fn division<'ast>(builder: &AstBuilder<'ast>, left: f64, right: f64) -> Expression<'ast> {
    Expression::new_binary_expression(
        SPAN,
        numeric_literal(builder, left),
        BinaryOperator::Division,
        numeric_literal(builder, right),
        builder,
    )
}

fn numeric_literal<'a>(builder: &AstBuilder<'a>, value: f64) -> Expression<'a> {
    Expression::new_numeric_literal(SPAN, value, None, NumberBase::Decimal, builder)
}

#[cfg(test)]
mod tests {
    use evrel_js_ir::{ConstantValue, JsString};
    use oxc_allocator::Allocator;
    use oxc_ast::AstBuilder;
    use oxc_codegen::Codegen;

    use super::emit_constant_expression;

    fn print(value: ConstantValue) -> String {
        let allocator = Allocator::default();
        let builder = AstBuilder::new(&allocator);
        let expression = emit_constant_expression(&builder, &value);
        let mut codegen = Codegen::new();

        codegen.print_expression(&expression);
        codegen.into_source_text()
    }

    #[test]
    fn emits_constants_without_shadowable_intrinsics() {
        assert_eq!(print(ConstantValue::Undefined), "void 0");
        assert_eq!(print(ConstantValue::Boolean(false)), "false");
        assert_eq!(print(ConstantValue::Boolean(true)), "true");
        assert_eq!(print(ConstantValue::Null), "null");

        assert_eq!(print(ConstantValue::Number(42.0)), "42");
        assert_eq!(print(ConstantValue::Number(-0.0)), "-0");
        assert_eq!(print(ConstantValue::Number(f64::NAN)), "0 / 0");
        assert_eq!(print(ConstantValue::Number(f64::INFINITY)), "1 / 0",);
        assert_eq!(print(ConstantValue::Number(f64::NEG_INFINITY)), "-1 / 0",);

        assert_eq!(print(ConstantValue::BigInt("42".into())), "42n",);

        assert_eq!(
            print(ConstantValue::String(JsString::new("evrel", false,))),
            "\"evrel\"",
        );
    }
}
