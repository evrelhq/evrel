//! Primitive two-result JavaScript numeric-update emission.

use evrel_ir::{OperationId, UpdateOp, UpdateOperator as IrUpdateOperator, ValueId};
use oxc_allocator::{GetAllocator, Vec as ArenaVec};
use oxc_ast::ast::{AssignmentTarget, Expression, SimpleAssignmentTarget, Statement};
use oxc_span::SPAN;
use oxc_syntax::operator::{AssignmentOperator, UpdateOperator as AstUpdateOperator};

use crate::{JsCodegenError, plan::JsValueRepresentation};

use super::{FunctionEmission, value::emit_value_expression};

pub(crate) fn emit_update_statement<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    operation: OperationId,
    update: &UpdateOp,
    operands: &[ValueId],
    results: &[ValueId],
) -> Result<Statement<'ast>, JsCodegenError> {
    let builder = emission.builder;
    let plan = emission.plan;

    let [current] = operands else {
        return Err(JsCodegenError::MalformedOperation { operation });
    };
    let [old_numeric, new_numeric] = results else {
        return Err(JsCodegenError::MalformedOperation { operation });
    };
    let Some(JsValueRepresentation::Temporary(old_local)) = plan.value(*old_numeric) else {
        return Err(JsCodegenError::UnsupportedValue {
            value: *old_numeric,
        });
    };
    let Some(JsValueRepresentation::Temporary(new_local)) = plan.value(*new_numeric) else {
        return Err(JsCodegenError::UnsupportedValue {
            value: *new_numeric,
        });
    };
    let old_name = builder
        .allocator()
        .alloc_str(
            plan.local_name(old_local)
                .ok_or(JsCodegenError::UnsupportedValue {
                    value: *old_numeric,
                })?,
        );
    let new_name = builder
        .allocator()
        .alloc_str(
            plan.local_name(new_local)
                .ok_or(JsCodegenError::UnsupportedValue {
                    value: *new_numeric,
                })?,
        );
    let initialize_new = Expression::new_assignment_expression(
        SPAN,
        AssignmentOperator::Assign,
        AssignmentTarget::new_assignment_target_identifier(SPAN, new_name, builder),
        emit_value_expression(builder, emission.function, plan, *current)?,
        builder,
    );
    let initialize_old = Expression::new_assignment_expression(
        SPAN,
        AssignmentOperator::Assign,
        AssignmentTarget::new_assignment_target_identifier(SPAN, old_name, builder),
        Expression::new_update_expression(
            SPAN,
            match update.operator() {
                IrUpdateOperator::Increment => AstUpdateOperator::Increment,
                IrUpdateOperator::Decrement => AstUpdateOperator::Decrement,
            },
            false,
            SimpleAssignmentTarget::new_assignment_target_identifier(SPAN, new_name, builder),
            builder,
        ),
        builder,
    );

    Ok(Statement::new_expression_statement(
        SPAN,
        Expression::new_sequence_expression(
            SPAN,
            ArenaVec::from_array_in([initialize_new, initialize_old], builder),
            builder,
        ),
        builder,
    ))
}
