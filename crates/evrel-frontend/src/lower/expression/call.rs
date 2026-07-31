//! JavaScript call-expression lowering.

use evrel_js_ir::{
    CallOp, CallReceiver, CallTarget, OperationKind, PropertyKey, SuperCallOp, SuperPropertyKey,
    ValueId,
};
use oxc_ast::ast::{Argument, CallExpression, Expression};

use crate::{FrontendError, lower::FunctionLowerer};

use super::{arguments::lower_arguments, lower_expression};

/// Lowers a JavaScript function or method call.
pub(super) fn lower_call_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    call: &CallExpression<'_>,
) -> Result<ValueId, FrontendError> {
    if call.optional {
        return Err(FrontendError::InvalidOptionalChain);
    }

    if matches!(&call.callee, Expression::Super(_)) {
        return lower_super_call(lowerer, &call.arguments, call.pure);
    }

    // The complete call target is evaluated before its arguments.
    let target = lower_call_target(lowerer, &call.callee)?;

    emit_call(lowerer, target, &call.arguments, call.pure)
}

fn lower_super_call(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    arguments: &[Argument<'_>],
    has_pure_annotation: bool,
) -> Result<ValueId, FrontendError> {
    let operation = SuperCallOp::new(lower_arguments(lowerer, arguments)?)
        .with_pure_annotation(has_pure_annotation);

    Ok(lowerer.emit_value(OperationKind::SuperCall(operation), []))
}

pub(super) struct LoweredCallTarget {
    pub(super) target: CallTarget,
    pub(super) operands: Vec<ValueId>,
}

impl LoweredCallTarget {
    pub(super) fn value(callee: ValueId, receiver: Option<ValueId>) -> Self {
        let mut operands = Vec::with_capacity(1 + usize::from(receiver.is_some()));
        operands.push(callee);

        let receiver = match receiver {
            Some(receiver) => {
                operands.push(receiver);
                CallReceiver::Explicit
            }
            None => CallReceiver::None,
        };

        Self {
            target: CallTarget::Value { receiver },
            operands,
        }
    }
}

pub(super) fn emit_call(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    target: LoweredCallTarget,
    arguments: &[Argument<'_>],
    has_pure_annotation: bool,
) -> Result<ValueId, FrontendError> {
    let operation = CallOp::new(target.target, lower_arguments(lowerer, arguments)?)
        .with_pure_annotation(has_pure_annotation);

    Ok(lowerer.emit_value(OperationKind::Call(operation), target.operands))
}

pub(super) fn lower_call_target(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    callee: &Expression<'_>,
) -> Result<LoweredCallTarget, FrontendError> {
    match callee {
        Expression::StaticMemberExpression(member)
            if matches!(&member.object, Expression::Super(_)) =>
        {
            Ok(LoweredCallTarget {
                target: CallTarget::SuperProperty(SuperPropertyKey::Static(
                    member.property.name.as_str().into(),
                )),
                operands: Vec::new(),
            })
        }
        Expression::ComputedMemberExpression(member)
            if matches!(&member.object, Expression::Super(_)) =>
        {
            let key = lower_expression(lowerer, &member.expression)?;

            Ok(LoweredCallTarget {
                target: CallTarget::SuperProperty(SuperPropertyKey::Computed),
                operands: vec![key],
            })
        }
        Expression::StaticMemberExpression(member) => {
            if member.optional {
                return Err(FrontendError::InvalidOptionalChain);
            }

            let receiver = lower_expression(lowerer, &member.object)?;

            Ok(LoweredCallTarget {
                target: CallTarget::Property(PropertyKey::Static(
                    member.property.name.as_str().into(),
                )),
                operands: vec![receiver],
            })
        }
        Expression::ComputedMemberExpression(member) => {
            if member.optional {
                return Err(FrontendError::InvalidOptionalChain);
            }

            // JavaScript evaluates the receiver and key before reading the method.
            let receiver = lower_expression(lowerer, &member.object)?;
            let key = lower_expression(lowerer, &member.expression)?;

            Ok(LoweredCallTarget {
                target: CallTarget::Property(PropertyKey::Computed),
                operands: vec![receiver, key],
            })
        }
        Expression::PrivateFieldExpression(member) => {
            if member.optional {
                return Err(FrontendError::InvalidOptionalChain);
            }

            let receiver = lower_expression(lowerer, &member.object)?;
            let private_name = lowerer.private_name(member.field.name.as_str());

            Ok(LoweredCallTarget {
                target: CallTarget::Property(PropertyKey::Private(private_name)),
                operands: vec![receiver],
            })
        }
        expression => Ok(LoweredCallTarget::value(
            lower_expression(lowerer, expression)?,
            None,
        )),
    }
}
