//! JavaScript unary-expression lowering.

use evrel_js_ir::{
    DeleteOp, DeleteTarget, OperationKind, PropertyKey, TypeofOp, UnaryOp, UnaryOperator, ValueId,
};
use oxc_ast::ast::{Expression, UnaryExpression};
use oxc_syntax::operator::UnaryOperator as OxcUnaryOperator;

use crate::{FrontendError, lower::FunctionLowerer};

use super::lower_expression;

/// Lowers a value-based JavaScript unary expression.
pub(super) fn lower_unary_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &UnaryExpression<'_>,
) -> Result<ValueId, FrontendError> {
    if expression.operator == OxcUnaryOperator::Typeof {
        return lower_typeof_expression(lowerer, expression);
    }

    if expression.operator == OxcUnaryOperator::Delete {
        return lower_delete_expression(lowerer, expression);
    }

    let operator = match expression.operator {
        OxcUnaryOperator::UnaryPlus => UnaryOperator::Plus,
        OxcUnaryOperator::UnaryNegation => UnaryOperator::Negate,
        OxcUnaryOperator::BitwiseNot => UnaryOperator::BitwiseNot,
        OxcUnaryOperator::LogicalNot => UnaryOperator::LogicalNot,
        OxcUnaryOperator::Void => UnaryOperator::Void,

        OxcUnaryOperator::Delete => unreachable!("delete is lowered separately"),
        OxcUnaryOperator::Typeof => unreachable!("typeof is lowered separately"),
    };

    let argument = lower_expression(lowerer, &expression.argument)?;

    Ok(lowerer.emit_value(OperationKind::Unary(UnaryOp::new(operator)), [argument]))
}

fn lower_typeof_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &UnaryExpression<'_>,
) -> Result<ValueId, FrontendError> {
    if let Expression::Identifier(identifier) = &expression.argument
        && lowerer.binding_for_reference(identifier).is_none()
        && !(identifier.name == "arguments" && lowerer.has_arguments_environment())
    {
        return Ok(lowerer.emit_value(
            OperationKind::Typeof(TypeofOp::global(identifier.name.as_str())),
            [],
        ));
    }

    let argument = lower_expression(lowerer, &expression.argument)?;

    Ok(lowerer.emit_value(OperationKind::Typeof(TypeofOp::value()), [argument]))
}

fn lower_delete_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &UnaryExpression<'_>,
) -> Result<ValueId, FrontendError> {
    match &expression.argument {
        Expression::StaticMemberExpression(member) => {
            if member.optional {
                return Err(FrontendError::InvalidOptionalChain);
            }

            let object = lower_expression(lowerer, &member.object)?;

            Ok(lowerer.emit_value(
                OperationKind::Delete(DeleteOp::new(DeleteTarget::Property(PropertyKey::Static(
                    member.property.name.as_str().into(),
                )))),
                [object],
            ))
        }
        Expression::ComputedMemberExpression(member) => {
            if member.optional {
                return Err(FrontendError::InvalidOptionalChain);
            }

            // JavaScript evaluates the object before the computed key.
            let object = lower_expression(lowerer, &member.object)?;
            let key = lower_expression(lowerer, &member.expression)?;

            Ok(lowerer.emit_value(
                OperationKind::Delete(DeleteOp::new(DeleteTarget::Property(PropertyKey::Computed))),
                [object, key],
            ))
        }
        Expression::ChainExpression(_) => Err(FrontendError::InvalidOptionalChain),
        argument => {
            let value = lower_expression(lowerer, argument)?;

            Ok(lowerer.emit_value(
                OperationKind::Delete(DeleteOp::new(DeleteTarget::Value)),
                [value],
            ))
        }
    }
}
