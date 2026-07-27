//! JavaScript regular-expression literal emission.

use evrel_ir::{OperationId, RegExpLiteralOp};
use oxc_allocator::GetAllocator;
use oxc_ast::{
    AstBuilder,
    ast::{Expression, RegExp, RegExpFlags, RegExpPattern},
};
use oxc_span::SPAN;

use crate::JsCodegenError;

/// Emits one JavaScript regular-expression literal.
pub(crate) fn emit_regexp_literal_expression<'ast>(
    builder: &AstBuilder<'ast>,
    operation: OperationId,
    literal: &RegExpLiteralOp,
) -> Result<Expression<'ast>, JsCodegenError> {
    let mut flags = RegExpFlags::empty();

    for flag in literal.flags().bytes() {
        let flag =
            RegExpFlags::try_from(flag).map_err(|_| JsCodegenError::UnsupportedOperation {
                operation,
                reason: concat!(file!(), ":", line!()),
            })?;

        if flags.contains(flag) {
            return Err(JsCodegenError::UnsupportedOperation {
                operation,
                reason: concat!(file!(), ":", line!()),
            });
        }

        flags.insert(flag);
    }

    if flags.contains(RegExpFlags::U) && flags.contains(RegExpFlags::V) {
        return Err(JsCodegenError::UnsupportedOperation {
            operation,
            reason: concat!(file!(), ":", line!()),
        });
    }

    let regex = RegExp {
        pattern: RegExpPattern {
            text: builder.allocator().alloc_str(literal.pattern()).into(),
            pattern: None,
        },
        flags,
    };

    Ok(Expression::new_reg_exp_literal(SPAN, regex, None, builder))
}
