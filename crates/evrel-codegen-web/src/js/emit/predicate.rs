//! JavaScript semantic-predicate emission.

use evrel_js_ir::{ConstantValue, HasPrivateNameOp, JsFunctionIr, JsModuleIr, ValueId};
use oxc_allocator::GetAllocator;
use oxc_ast::{
    AstBuilder,
    ast::{BinaryOperator, Expression, PrivateIdentifier},
};
use oxc_span::SPAN;
use oxc_syntax::operator::LogicalOperator;

use crate::{JsCodegenError, js::plan::JsFunctionPlan};

use super::{constant::emit_constant_expression, value::emit_value_expression};

/// Emits an exact JavaScript nullish test.
///
/// Loose equality cannot be used because `document.all` is loosely equal
/// to null even though it is not nullish.
pub(crate) fn emit_is_nullish_expression<'ast>(
    builder: &AstBuilder<'ast>,
    function: &JsFunctionIr,
    plan: &JsFunctionPlan,
    value: ValueId,
) -> Result<Expression<'ast>, JsCodegenError> {
    let null_check = Expression::new_binary_expression(
        SPAN,
        emit_value_expression(builder, function, plan, value)?,
        BinaryOperator::StrictEquality,
        emit_constant_expression(builder, &ConstantValue::Null),
        builder,
    );

    let undefined_check = Expression::new_binary_expression(
        SPAN,
        emit_value_expression(builder, function, plan, value)?,
        BinaryOperator::StrictEquality,
        emit_constant_expression(builder, &ConstantValue::Undefined),
        builder,
    );

    Ok(Expression::new_logical_expression(
        SPAN,
        null_check,
        LogicalOperator::Or,
        undefined_check,
        builder,
    ))
}

pub(crate) fn emit_has_private_name_expression<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    function: &JsFunctionIr,
    plan: &JsFunctionPlan,
    operation: &HasPrivateNameOp,
    value: ValueId,
) -> Result<Expression<'ast>, JsCodegenError> {
    let private_name = module.private_name(operation.private_name()).ok_or(
        JsCodegenError::UnknownPrivateName {
            private_name: operation.private_name(),
        },
    )?;

    Ok(Expression::new_private_in_expression(
        SPAN,
        PrivateIdentifier::new(
            SPAN,
            builder.allocator().alloc_str(private_name.name()),
            builder,
        ),
        emit_value_expression(builder, function, plan, value)?,
        builder,
    ))
}
