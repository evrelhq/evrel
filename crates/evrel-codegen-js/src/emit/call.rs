//! Conservative JavaScript invocation emission.

use evrel_ir::{
    CallArgument, CallOp, CallReceiver, CallTarget, ConstructOp, OperationId, PropertyKey,
    SuperCallOp, SuperPropertyKey, ValueId,
};
use oxc_allocator::{Box as ArenaBox, GetAllocator, Vec as ArenaVec};
use oxc_ast::ast::{Argument, Expression, TSTypeParameterInstantiation};
use oxc_span::SPAN;

use crate::JsCodegenError;

use super::{
    FunctionEmission,
    property::{
        emit_computed_member_expression, emit_private_member_expression,
        emit_static_member_expression,
    },
    region::emit_expression_region,
    value::emit_value_expression,
};

pub(crate) fn emit_call_expression<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    operation_id: OperationId,
    operation: &CallOp,
    operands: &[ValueId],
) -> Result<Expression<'ast>, JsCodegenError> {
    let builder = emission.builder;
    let function = emission.function;

    if matches!(
        operation.target(),
        CallTarget::Value {
            receiver: CallReceiver::Explicit
        }
    ) {
        let [callee, receiver] = operands else {
            return Err(JsCodegenError::MalformedOperation {
                operation: operation_id,
            });
        };
        let callee = emit_value_expression(builder, function, emission.plan, *callee)?;
        let receiver = emit_value_expression(builder, function, emission.plan, *receiver)?;
        let mut arguments = emit_arguments(emission, operation.arguments())?;
        arguments.insert(0, Argument::from(receiver));
        arguments.insert(0, Argument::from(callee));

        let function =
            Expression::new_identifier(SPAN, builder.allocator().alloc_str("Function"), builder);
        let prototype = Expression::from(emit_static_member_expression(
            builder,
            function,
            "prototype",
        ));
        let call = Expression::from(emit_static_member_expression(builder, prototype, "call"));
        let call_call = Expression::from(emit_static_member_expression(builder, call, "call"));

        return Ok(Expression::new_call_expression_with_pure(
            SPAN,
            call_call,
            None::<ArenaBox<'ast, TSTypeParameterInstantiation<'ast>>>,
            arguments,
            false,
            operation.has_pure_annotation(),
            builder,
        ));
    }

    let callee = emit_call_target(emission, operation_id, operation.target(), operands)?;
    let arguments = emit_arguments(emission, operation.arguments())?;

    Ok(Expression::new_call_expression_with_pure(
        SPAN,
        callee,
        None::<ArenaBox<'ast, TSTypeParameterInstantiation<'ast>>>,
        arguments,
        false,
        operation.has_pure_annotation(),
        builder,
    ))
}

pub(crate) fn emit_call_target<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    operation_id: OperationId,
    target: &CallTarget,
    operands: &[ValueId],
) -> Result<Expression<'ast>, JsCodegenError> {
    let builder = emission.builder;
    let module = emission.module;
    let function = emission.function;
    let plan = emission.plan;

    Ok(match target {
        CallTarget::Value {
            receiver: CallReceiver::None,
        } => {
            let [callee] = operands else {
                return Err(JsCodegenError::MalformedOperation {
                    operation: operation_id,
                });
            };

            emit_value_expression(builder, function, plan, *callee)?
        }
        CallTarget::Property(PropertyKey::Static(name)) => {
            let [object] = operands else {
                return Err(JsCodegenError::MalformedOperation {
                    operation: operation_id,
                });
            };
            let object = emit_value_expression(builder, function, plan, *object)?;
            Expression::from(emit_static_member_expression(builder, object, name))
        }
        CallTarget::Property(PropertyKey::Computed) => {
            let [object, key] = operands else {
                return Err(JsCodegenError::MalformedOperation {
                    operation: operation_id,
                });
            };
            let object = emit_value_expression(builder, function, plan, *object)?;
            let key = emit_value_expression(builder, function, plan, *key)?;
            Expression::from(emit_computed_member_expression(builder, object, key))
        }
        CallTarget::Property(PropertyKey::Private(private_name)) => {
            let [object] = operands else {
                return Err(JsCodegenError::MalformedOperation {
                    operation: operation_id,
                });
            };
            let object = emit_value_expression(builder, function, plan, *object)?;

            Expression::from(emit_private_member_expression(
                builder,
                module,
                object,
                *private_name,
            )?)
        }
        CallTarget::SuperProperty(SuperPropertyKey::Static(name)) => {
            let [] = operands else {
                return Err(JsCodegenError::MalformedOperation {
                    operation: operation_id,
                });
            };
            Expression::from(emit_static_member_expression(
                builder,
                Expression::new_super(SPAN, builder),
                name,
            ))
        }
        CallTarget::SuperProperty(SuperPropertyKey::Computed) => {
            let [key] = operands else {
                return Err(JsCodegenError::MalformedOperation {
                    operation: operation_id,
                });
            };
            let key = emit_value_expression(builder, function, plan, *key)?;
            Expression::from(emit_computed_member_expression(
                builder,
                Expression::new_super(SPAN, builder),
                key,
            ))
        }
        CallTarget::Value {
            receiver: CallReceiver::Explicit,
        } => {
            return Err(JsCodegenError::UnsupportedOperation {
                operation: operation_id,
                reason: concat!(file!(), ":", line!()),
            });
        }
    })
}

pub(crate) fn emit_super_call_expression<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    operation: &SuperCallOp,
) -> Result<Expression<'ast>, JsCodegenError> {
    let builder = emission.builder;
    let arguments = emit_arguments(emission, operation.arguments())?;

    Ok(Expression::new_call_expression_with_pure(
        SPAN,
        Expression::new_super(SPAN, builder),
        None::<ArenaBox<'ast, TSTypeParameterInstantiation<'ast>>>,
        arguments,
        false,
        operation.has_pure_annotation(),
        builder,
    ))
}

pub(crate) fn emit_construct_expression<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    operation_id: OperationId,
    operation: &ConstructOp,
    operands: &[ValueId],
) -> Result<Expression<'ast>, JsCodegenError> {
    let builder = emission.builder;

    let [constructor] = operands else {
        return Err(JsCodegenError::MalformedOperation {
            operation: operation_id,
        });
    };
    let constructor =
        emit_value_expression(builder, emission.function, emission.plan, *constructor)?;
    let arguments = emit_arguments(emission, operation.arguments())?;

    Ok(Expression::new_new_expression_with_pure(
        SPAN,
        constructor,
        None::<ArenaBox<'ast, TSTypeParameterInstantiation<'ast>>>,
        arguments,
        operation.has_pure_annotation(),
        builder,
    ))
}

pub(super) fn emit_arguments<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    source: &[CallArgument],
) -> Result<ArenaVec<'ast, Argument<'ast>>, JsCodegenError> {
    let builder = emission.builder;
    let mut arguments = ArenaVec::with_capacity_in(source.len(), builder);
    for argument in source {
        let expression = emit_expression_region(
            builder,
            emission.module,
            emission.output_plan,
            emission.function,
            emission.plan,
            argument.expression(),
        )?;
        arguments.push(match argument {
            CallArgument::Value { .. } => Argument::from(expression),
            CallArgument::Spread { .. } => Argument::new_spread_element(SPAN, expression, builder),
        });
    }
    Ok(arguments)
}
