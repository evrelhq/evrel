//! JavaScript literal expression lowering.

use evrel_js_ir::{ConstantOp, ConstantValue, JsString, OperationKind, RegExpLiteralOp, ValueId};
use oxc_ast::ast::{
    BigIntLiteral, BooleanLiteral, NullLiteral, NumericLiteral, RegExpLiteral, StringLiteral,
};

use crate::lower::FunctionLowerer;

/// Lowers an ECMAScript boolean literal.
pub(super) fn lower_boolean_literal(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    literal: &BooleanLiteral,
) -> ValueId {
    lowerer.emit_value(
        OperationKind::Constant(ConstantOp::new(ConstantValue::Boolean(literal.value))),
        [],
    )
}

/// Lowers the ECMAScript null literal.
pub(super) fn lower_null_literal(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    _literal: &NullLiteral,
) -> ValueId {
    lowerer.emit_value(
        OperationKind::Constant(ConstantOp::new(ConstantValue::Null)),
        [],
    )
}

/// Lowers an ECMAScript numeric literal.
pub(super) fn lower_numeric_literal(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    literal: &NumericLiteral<'_>,
) -> ValueId {
    lowerer.emit_value(
        OperationKind::Constant(ConstantOp::new(ConstantValue::Number(literal.value))),
        [],
    )
}

/// Lowers an ECMAScript BigInt literal.
pub(super) fn lower_bigint_literal(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    literal: &BigIntLiteral<'_>,
) -> ValueId {
    lowerer.emit_value(
        OperationKind::Constant(ConstantOp::new(ConstantValue::BigInt(
            literal.value.as_str().into(),
        ))),
        [],
    )
}

/// Lowers an ECMAScript regular-expression literal.
pub(super) fn lower_regexp_literal(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    literal: &RegExpLiteral<'_>,
) -> ValueId {
    lowerer.emit_value(
        OperationKind::RegExpLiteral(RegExpLiteralOp::new(
            literal.regex.pattern.text.as_str(),
            literal.regex.flags.to_string(),
        )),
        [],
    )
}

/// Lowers an ECMAScript string literal.
pub(super) fn lower_string_literal(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    literal: &StringLiteral<'_>,
) -> ValueId {
    let value = JsString::new(literal.value.as_str(), literal.lone_surrogates);

    lowerer.emit_value(
        OperationKind::Constant(ConstantOp::new(ConstantValue::String(value))),
        [],
    )
}
