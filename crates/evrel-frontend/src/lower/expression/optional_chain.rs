//! JavaScript optional-chain lowering.

use evrel_ir::{
    BlockId, BlockTarget, CallTarget, ConstantOp, ConstantValue, IfOp, IsNullishOp, JumpOp,
    LoadPropertyOp, OperationKind, PropertyKey, ValueId,
};
use oxc_ast::ast::{
    CallExpression, ChainElement, ChainExpression, ComputedMemberExpression, Expression,
    PrivateFieldExpression, StaticMemberExpression,
};

use crate::{FrontendError, lower::FunctionLowerer};

use super::{call, lower_expression, member::MemberRead};

#[derive(Clone, Copy)]
struct OptionalChainExit {
    block: BlockId,
    undefined: ValueId,
}

/// Lowers one continuous optional chain.
pub(super) fn lower_optional_chain(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    chain: &ChainExpression<'_>,
) -> Result<ValueId, FrontendError> {
    let exit_block = lowerer.create_block();
    let result = lowerer.append_forwarded_block_parameter(exit_block);
    let undefined = lowerer.emit_value(
        OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
        [],
    );
    let exit = OptionalChainExit {
        block: exit_block,
        undefined,
    };

    let value = match &chain.expression {
        ChainElement::StaticMemberExpression(member) => {
            lower_static_member_read(lowerer, member, exit)?.value
        }
        ChainElement::ComputedMemberExpression(member) => {
            lower_computed_member_read(lowerer, member, exit)?.value
        }
        ChainElement::PrivateFieldExpression(member) => {
            lower_private_member_read(lowerer, member, exit)?.value
        }
        ChainElement::CallExpression(call) => lower_chain_call(lowerer, call, exit)?,
        _ => return Err(FrontendError::InvalidOptionalChain),
    };

    if !lowerer.current_block_is_terminated() {
        lowerer.terminate(
            OperationKind::Jump(JumpOp::new(BlockTarget::new(exit_block, 1))),
            [value],
        );
    }

    lowerer.switch_to_block(exit_block);

    Ok(result)
}

fn lower_static_member_read(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    member: &StaticMemberExpression<'_>,
    exit: OptionalChainExit,
) -> Result<MemberRead, FrontendError> {
    let receiver = lower_chain_value(lowerer, &member.object, exit)?;

    if member.optional {
        short_circuit_if_nullish(lowerer, receiver, exit);
    }

    let value = lowerer.emit_value(
        OperationKind::LoadProperty(LoadPropertyOp::new(PropertyKey::Static(
            member.property.name.as_str().into(),
        ))),
        [receiver],
    );

    Ok(MemberRead { value, receiver })
}

fn lower_computed_member_read(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    member: &ComputedMemberExpression<'_>,
    exit: OptionalChainExit,
) -> Result<MemberRead, FrontendError> {
    let receiver = lower_chain_value(lowerer, &member.object, exit)?;

    if member.optional {
        short_circuit_if_nullish(lowerer, receiver, exit);
    }

    // The key must not run when the receiver short-circuits.
    let key = lower_expression(lowerer, &member.expression)?;

    let value = lowerer.emit_value(
        OperationKind::LoadProperty(LoadPropertyOp::new(PropertyKey::Computed)),
        [receiver, key],
    );

    Ok(MemberRead { value, receiver })
}

fn lower_private_member_read(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    member: &PrivateFieldExpression<'_>,
    exit: OptionalChainExit,
) -> Result<MemberRead, FrontendError> {
    let receiver = lower_chain_value(lowerer, &member.object, exit)?;

    if member.optional {
        short_circuit_if_nullish(lowerer, receiver, exit);
    }

    let private_name = lowerer.private_name(member.field.name.as_str());
    let value = lowerer.emit_value(
        OperationKind::LoadProperty(LoadPropertyOp::new(PropertyKey::Private(private_name))),
        [receiver],
    );

    Ok(MemberRead { value, receiver })
}

fn lower_chain_value(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &Expression<'_>,
    exit: OptionalChainExit,
) -> Result<ValueId, FrontendError> {
    match expression {
        Expression::StaticMemberExpression(member) => {
            Ok(lower_static_member_read(lowerer, member, exit)?.value)
        }
        Expression::ComputedMemberExpression(member) => {
            Ok(lower_computed_member_read(lowerer, member, exit)?.value)
        }
        Expression::PrivateFieldExpression(member) => {
            Ok(lower_private_member_read(lowerer, member, exit)?.value)
        }
        Expression::CallExpression(call) => lower_chain_call(lowerer, call, exit),
        expression => lower_expression(lowerer, expression),
    }
}

fn lower_chain_call(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &CallExpression<'_>,
    exit: OptionalChainExit,
) -> Result<ValueId, FrontendError> {
    if !expression.optional {
        let target = match &expression.callee {
            Expression::StaticMemberExpression(member) => {
                let receiver = lower_chain_value(lowerer, &member.object, exit)?;

                if member.optional {
                    short_circuit_if_nullish(lowerer, receiver, exit);
                }

                Some(call::LoweredCallTarget {
                    target: CallTarget::Property(PropertyKey::Static(
                        member.property.name.as_str().into(),
                    )),
                    operands: vec![receiver],
                })
            }

            Expression::ComputedMemberExpression(member) => {
                let receiver = lower_chain_value(lowerer, &member.object, exit)?;

                if member.optional {
                    short_circuit_if_nullish(lowerer, receiver, exit);
                }

                // The key must not run when the receiver short-circuits.
                let key = lower_expression(lowerer, &member.expression)?;

                Some(call::LoweredCallTarget {
                    target: CallTarget::Property(PropertyKey::Computed),
                    operands: vec![receiver, key],
                })
            }

            Expression::PrivateFieldExpression(member) => {
                let receiver = lower_chain_value(lowerer, &member.object, exit)?;

                if member.optional {
                    short_circuit_if_nullish(lowerer, receiver, exit);
                }

                Some(call::LoweredCallTarget {
                    target: CallTarget::Property(PropertyKey::Private(
                        lowerer.private_name(member.field.name.as_str()),
                    )),
                    operands: vec![receiver],
                })
            }

            _ => None,
        };

        if let Some(target) = target {
            return call::emit_call(lowerer, target, &expression.arguments, expression.pure);
        }
    }

    let (callee, receiver) = match &expression.callee {
        Expression::StaticMemberExpression(member) => {
            let read = lower_static_member_read(lowerer, member, exit)?;

            (read.value, Some(read.receiver))
        }
        Expression::ComputedMemberExpression(member) => {
            let read = lower_computed_member_read(lowerer, member, exit)?;

            (read.value, Some(read.receiver))
        }
        Expression::PrivateFieldExpression(member) => {
            let read = lower_private_member_read(lowerer, member, exit)?;

            (read.value, Some(read.receiver))
        }
        callee => (lower_chain_value(lowerer, callee, exit)?, None),
    };

    if expression.optional {
        short_circuit_if_nullish(lowerer, callee, exit);
    }

    call::emit_call(
        lowerer,
        call::LoweredCallTarget::value(callee, receiver),
        &expression.arguments,
        expression.pure,
    )
}

fn short_circuit_if_nullish(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    value: ValueId,
    exit: OptionalChainExit,
) {
    let continuation_block = lowerer.create_block();
    let is_nullish = lowerer.emit_value(OperationKind::IsNullish(IsNullishOp::new()), [value]);

    lowerer.terminate(
        OperationKind::If(IfOp::new(
            BlockTarget::new(exit.block, 1),
            BlockTarget::new(continuation_block, 0),
            exit.block,
        )),
        [is_nullish, exit.undefined],
    );

    lowerer.switch_to_block(continuation_block);
}
