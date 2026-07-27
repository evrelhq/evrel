//! JavaScript assignment-expression lowering.

use evrel_ir::{
    BinaryOp, BinaryOperator, BindingId, BlockTarget, IfOp, IsNullishOp, JumpOp, LoadBindingOp,
    LoadGlobalOp, LoadPropertyOp, LoadSuperPropertyOp, OperationKind, PrivateNameId, PropertyKey,
    StoreBindingOp, StoreGlobalOp, StorePropertyOp, StoreSuperPropertyOp, SuperPropertyKey,
    ValueId,
};
use oxc_ast::ast::{
    AssignmentExpression, AssignmentTarget as OxcAssignmentTarget, Expression,
    SimpleAssignmentTarget,
};
use oxc_syntax::operator::AssignmentOperator;

use crate::{
    FrontendError,
    lower::{
        FunctionLowerer,
        pattern::{emit_assignment_pattern_write, lower_assignment_pattern},
    },
};

use super::lower_expression;

/// An evaluated JavaScript assignment reference.
pub(super) enum AssignmentReference {
    Binding(BindingId),

    Global {
        name: Box<str>,
    },

    StaticProperty {
        object: ValueId,
        name: Box<str>,
    },

    ComputedProperty {
        object: ValueId,
        key: ValueId,
    },

    PrivateProperty {
        object: ValueId,
        private_name: PrivateNameId,
    },

    StaticSuperProperty {
        name: Box<str>,
    },

    ComputedSuperProperty {
        key: ValueId,
    },
}

/// Lowers a JavaScript simple or compound assignment expression.
pub(super) fn lower_assignment_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &AssignmentExpression<'_>,
) -> Result<ValueId, FrontendError> {
    if expression.left.as_assignment_target_pattern().is_some() {
        // Destructuring evaluates the RHS before entering the pattern.
        let value = lower_expression(lowerer, &expression.right)?;
        let pattern = lower_assignment_pattern(lowerer, &expression.left)?;

        emit_assignment_pattern_write(lowerer, pattern, value);

        // Like ordinary assignment, destructuring evaluates to its RHS value.
        return Ok(value);
    }

    // JavaScript evaluates the complete left-hand reference before the RHS.
    let reference = lower_assignment_reference(lowerer, &expression.left)?;

    if matches!(
        expression.operator,
        AssignmentOperator::LogicalAnd
            | AssignmentOperator::LogicalOr
            | AssignmentOperator::LogicalNullish
    ) {
        return lower_logical_assignment(lowerer, expression, reference);
    }

    let value = if expression.operator == AssignmentOperator::Assign {
        lower_expression(lowerer, &expression.right)?
    } else {
        let operator = binary_operator_for_assignment(expression.operator)?;
        let current = load_assignment_reference(lowerer, &reference);
        let right = lower_expression(lowerer, &expression.right)?;

        lowerer.emit_value(
            OperationKind::Binary(BinaryOp::new(operator)),
            [current, right],
        )
    };

    store_assignment_reference(lowerer, reference, value);

    // Assignment expressions evaluate to the assigned value.
    Ok(value)
}

/// Evaluates an assignment target and stores one value through it.
pub(in crate::lower) fn lower_assignment_target_write(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    target: &OxcAssignmentTarget<'_>,
    value: ValueId,
) -> Result<(), FrontendError> {
    if target.as_assignment_target_pattern().is_some() {
        let pattern = lower_assignment_pattern(lowerer, target)?;

        emit_assignment_pattern_write(lowerer, pattern, value);

        return Ok(());
    }

    let reference = lower_assignment_reference(lowerer, target)?;

    store_assignment_reference(lowerer, reference, value);

    Ok(())
}

fn lower_logical_assignment(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &AssignmentExpression<'_>,
    reference: AssignmentReference,
) -> Result<ValueId, FrontendError> {
    let current = load_assignment_reference(lowerer, &reference);

    let assignment_block = lowerer.create_block();
    let completion_block = lowerer.create_block();
    let result = lowerer.append_forwarded_block_parameter(completion_block);

    let (condition, then_target, else_target) = match expression.operator {
        AssignmentOperator::LogicalAnd => (
            current,
            BlockTarget::new(assignment_block, 0),
            BlockTarget::new(completion_block, 1),
        ),
        AssignmentOperator::LogicalOr => (
            current,
            BlockTarget::new(completion_block, 1),
            BlockTarget::new(assignment_block, 0),
        ),
        AssignmentOperator::LogicalNullish => {
            let is_nullish =
                lowerer.emit_value(OperationKind::IsNullish(IsNullishOp::new()), [current]);

            (
                is_nullish,
                BlockTarget::new(assignment_block, 0),
                BlockTarget::new(completion_block, 1),
            )
        }
        _ => unreachable!("expected a logical assignment operator"),
    };

    lowerer.terminate(
        OperationKind::If(IfOp::new(then_target, else_target, completion_block)),
        [condition, current],
    );

    lowerer.switch_to_block(assignment_block);

    let assigned = lower_expression(lowerer, &expression.right)?;
    store_assignment_reference(lowerer, reference, assigned);

    lowerer.terminate(
        OperationKind::Jump(JumpOp::new(BlockTarget::new(completion_block, 1))),
        [assigned],
    );

    lowerer.switch_to_block(completion_block);

    Ok(result)
}

pub(super) fn load_assignment_reference(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    reference: &AssignmentReference,
) -> ValueId {
    match reference {
        AssignmentReference::Binding(binding) => {
            lowerer.emit_value(OperationKind::LoadBinding(LoadBindingOp::new(*binding)), [])
        }
        AssignmentReference::Global { name } => lowerer.emit_value(
            OperationKind::LoadGlobal(LoadGlobalOp::new(name.clone())),
            [],
        ),
        AssignmentReference::StaticProperty { object, name } => lowerer.emit_value(
            OperationKind::LoadProperty(LoadPropertyOp::new(PropertyKey::Static(name.clone()))),
            [*object],
        ),
        AssignmentReference::ComputedProperty { object, key } => lowerer.emit_value(
            OperationKind::LoadProperty(LoadPropertyOp::new(PropertyKey::Computed)),
            [*object, *key],
        ),
        AssignmentReference::PrivateProperty {
            object,
            private_name,
        } => lowerer.emit_value(
            OperationKind::LoadProperty(LoadPropertyOp::new(PropertyKey::Private(*private_name))),
            [*object],
        ),
        AssignmentReference::StaticSuperProperty { name } => lowerer.emit_value(
            OperationKind::LoadSuperProperty(LoadSuperPropertyOp::new(SuperPropertyKey::Static(
                name.clone(),
            ))),
            [],
        ),
        AssignmentReference::ComputedSuperProperty { key } => lowerer.emit_value(
            OperationKind::LoadSuperProperty(LoadSuperPropertyOp::new(SuperPropertyKey::Computed)),
            [*key],
        ),
    }
}

fn binary_operator_for_assignment(
    operator: AssignmentOperator,
) -> Result<BinaryOperator, FrontendError> {
    match operator {
        AssignmentOperator::Addition => Ok(BinaryOperator::Add),
        AssignmentOperator::Subtraction => Ok(BinaryOperator::Subtract),
        AssignmentOperator::Multiplication => Ok(BinaryOperator::Multiply),
        AssignmentOperator::Division => Ok(BinaryOperator::Divide),
        AssignmentOperator::Remainder => Ok(BinaryOperator::Remainder),
        AssignmentOperator::Exponential => Ok(BinaryOperator::Exponentiate),
        AssignmentOperator::ShiftLeft => Ok(BinaryOperator::ShiftLeft),
        AssignmentOperator::ShiftRight => Ok(BinaryOperator::ShiftRight),
        AssignmentOperator::ShiftRightZeroFill => Ok(BinaryOperator::UnsignedShiftRight),
        AssignmentOperator::BitwiseOR => Ok(BinaryOperator::BitwiseOr),
        AssignmentOperator::BitwiseXOR => Ok(BinaryOperator::BitwiseXor),
        AssignmentOperator::BitwiseAnd => Ok(BinaryOperator::BitwiseAnd),
        _ => Err(FrontendError::UnsupportedAssignmentOperator),
    }
}

fn lower_assignment_reference(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    target: &OxcAssignmentTarget<'_>,
) -> Result<AssignmentReference, FrontendError> {
    let target = target
        .as_simple_assignment_target()
        .ok_or(FrontendError::UnsupportedAssignmentTarget)?;

    lower_simple_assignment_reference(lowerer, target)
}

pub(super) fn lower_simple_assignment_reference(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    target: &SimpleAssignmentTarget<'_>,
) -> Result<AssignmentReference, FrontendError> {
    match target {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            Ok(match lowerer.binding_for_reference(identifier) {
                Some(binding) => AssignmentReference::Binding(binding),
                None => AssignmentReference::Global {
                    name: identifier.name.as_str().into(),
                },
            })
        }
        SimpleAssignmentTarget::StaticMemberExpression(member) => {
            if member.optional {
                return Err(FrontendError::InvalidOptionalChain);
            }

            if matches!(&member.object, Expression::Super(_)) {
                return Ok(AssignmentReference::StaticSuperProperty {
                    name: member.property.name.as_str().into(),
                });
            }

            let object = lower_expression(lowerer, &member.object)?;

            Ok(AssignmentReference::StaticProperty {
                object,
                name: member.property.name.as_str().into(),
            })
        }
        SimpleAssignmentTarget::ComputedMemberExpression(member) => {
            if member.optional {
                return Err(FrontendError::InvalidOptionalChain);
            }

            if matches!(&member.object, Expression::Super(_)) {
                let key = lower_expression(lowerer, &member.expression)?;

                return Ok(AssignmentReference::ComputedSuperProperty { key });
            }

            let object = lower_expression(lowerer, &member.object)?;
            let key = lower_expression(lowerer, &member.expression)?;

            Ok(AssignmentReference::ComputedProperty { object, key })
        }
        SimpleAssignmentTarget::PrivateFieldExpression(member) => {
            if member.optional {
                return Err(FrontendError::InvalidOptionalChain);
            }

            let object = lower_expression(lowerer, &member.object)?;
            let private_name = lowerer.private_name(member.field.name.as_str());

            Ok(AssignmentReference::PrivateProperty {
                object,
                private_name,
            })
        }
        _ => Err(FrontendError::UnsupportedAssignmentTarget),
    }
}

pub(super) fn store_assignment_reference(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    reference: AssignmentReference,
    value: ValueId,
) {
    match reference {
        AssignmentReference::Binding(binding) => {
            lowerer.emit(
                OperationKind::StoreBinding(StoreBindingOp::new(binding)),
                [value],
            );
        }
        AssignmentReference::Global { name } => {
            lowerer.emit(
                OperationKind::StoreGlobal(StoreGlobalOp::new(name)),
                [value],
            );
        }
        AssignmentReference::StaticProperty { object, name } => {
            lowerer.emit(
                OperationKind::StoreProperty(StorePropertyOp::new(PropertyKey::Static(name))),
                [object, value],
            );
        }
        AssignmentReference::ComputedProperty { object, key } => {
            lowerer.emit(
                OperationKind::StoreProperty(StorePropertyOp::new(PropertyKey::Computed)),
                [object, key, value],
            );
        }
        AssignmentReference::PrivateProperty {
            object,
            private_name,
        } => {
            lowerer.emit(
                OperationKind::StoreProperty(StorePropertyOp::new(PropertyKey::Private(
                    private_name,
                ))),
                [object, value],
            );
        }
        AssignmentReference::StaticSuperProperty { name } => {
            lowerer.emit(
                OperationKind::StoreSuperProperty(StoreSuperPropertyOp::new(
                    SuperPropertyKey::Static(name),
                )),
                [value],
            );
        }
        AssignmentReference::ComputedSuperProperty { key } => {
            lowerer.emit(
                OperationKind::StoreSuperProperty(StoreSuperPropertyOp::new(
                    SuperPropertyKey::Computed,
                )),
                [key, value],
            );
        }
    }
}
