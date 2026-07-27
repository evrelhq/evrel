//! JavaScript member-expression lowering.

use evrel_ir::{
    LoadPropertyOp, LoadSuperPropertyOp, OperationKind, PropertyKey, SuperPropertyKey, ValueId,
};
use oxc_ast::ast::{
    ComputedMemberExpression, Expression, PrivateFieldExpression, StaticMemberExpression,
};

use crate::{FrontendError, lower::FunctionLowerer};

use super::lower_expression;

/// The result of reading a property, including the object used as its receiver.
#[derive(Debug, Clone, Copy)]
pub(super) struct MemberRead {
    pub(super) value: ValueId,
    pub(super) receiver: ValueId,
}

/// Lowers a property read whose name is known statically.
pub(super) fn lower_static_member_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    member: &StaticMemberExpression<'_>,
) -> Result<ValueId, FrontendError> {
    if matches!(&member.object, Expression::Super(_)) {
        return Ok(lowerer.emit_value(
            OperationKind::LoadSuperProperty(LoadSuperPropertyOp::new(SuperPropertyKey::Static(
                member.property.name.as_str().into(),
            ))),
            [],
        ));
    }

    Ok(lower_static_member_read(lowerer, member)?.value)
}

/// Lowers a property read whose key is computed at runtime.
pub(super) fn lower_computed_member_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    member: &ComputedMemberExpression<'_>,
) -> Result<ValueId, FrontendError> {
    if matches!(&member.object, Expression::Super(_)) {
        let key = lower_expression(lowerer, &member.expression)?;

        return Ok(lowerer.emit_value(
            OperationKind::LoadSuperProperty(LoadSuperPropertyOp::new(SuperPropertyKey::Computed)),
            [key],
        ));
    }

    Ok(lower_computed_member_read(lowerer, member)?.value)
}

/// Lowers a class-private property read.
pub(super) fn lower_private_member_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    member: &PrivateFieldExpression<'_>,
) -> Result<ValueId, FrontendError> {
    Ok(lower_private_member_read(lowerer, member)?.value)
}

/// Lowers a static property read while preserving its receiver.
pub(super) fn lower_static_member_read(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    member: &StaticMemberExpression<'_>,
) -> Result<MemberRead, FrontendError> {
    if member.optional {
        return Err(FrontendError::InvalidOptionalChain);
    }

    let receiver = lower_expression(lowerer, &member.object)?;
    let value = lowerer.emit_value(
        OperationKind::LoadProperty(LoadPropertyOp::new(PropertyKey::Static(
            member.property.name.as_str().into(),
        ))),
        [receiver],
    );

    Ok(MemberRead { value, receiver })
}

/// Lowers a computed property read while preserving its receiver.
pub(super) fn lower_computed_member_read(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    member: &ComputedMemberExpression<'_>,
) -> Result<MemberRead, FrontendError> {
    if member.optional {
        return Err(FrontendError::InvalidOptionalChain);
    }

    // JavaScript evaluates the object before the computed property key.
    let receiver = lower_expression(lowerer, &member.object)?;
    let key = lower_expression(lowerer, &member.expression)?;
    let value = lowerer.emit_value(
        OperationKind::LoadProperty(LoadPropertyOp::new(PropertyKey::Computed)),
        [receiver, key],
    );

    Ok(MemberRead { value, receiver })
}

/// Lowers a class-private property read while preserving its receiver.
pub(super) fn lower_private_member_read(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    member: &PrivateFieldExpression<'_>,
) -> Result<MemberRead, FrontendError> {
    if member.optional {
        return Err(FrontendError::InvalidOptionalChain);
    }

    let receiver = lower_expression(lowerer, &member.object)?;
    let private_name = lowerer.private_name(member.field.name.as_str());
    let value = lowerer.emit_value(
        OperationKind::LoadProperty(LoadPropertyOp::new(PropertyKey::Private(private_name))),
        [receiver],
    );

    Ok(MemberRead { value, receiver })
}
