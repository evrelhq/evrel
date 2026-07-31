//! Planned IR value emission.

use evrel_js_ir::{JsFunctionIr, OperationKind, ValueDefinition, ValueId};
use oxc_allocator::GetAllocator;
use oxc_ast::{AstBuilder, ast::Expression};
use oxc_span::SPAN;

use crate::{
    JsCodegenError,
    js::plan::{JsFunctionPlan, JsValueRepresentation},
};

use super::{
    constant::emit_constant_expression,
    context::{
        emit_load_arguments_expression, emit_load_this_expression, emit_meta_property_expression,
    },
};

/// Emits one IR value according to its planned representation.
pub(crate) fn emit_value_expression<'ast>(
    builder: &AstBuilder<'ast>,
    function: &JsFunctionIr,
    plan: &JsFunctionPlan,
    value: ValueId,
) -> Result<Expression<'ast>, JsCodegenError> {
    match plan.value(value) {
        Some(JsValueRepresentation::Temporary(local)) => {
            let name = plan
                .local_name(local)
                .ok_or(JsCodegenError::UnsupportedValue { value })?;

            Ok(Expression::new_identifier(
                SPAN,
                builder.allocator().alloc_str(name),
                builder,
            ))
        }

        Some(JsValueRepresentation::Binding(binding)) => {
            let name = plan
                .binding_name(binding)
                .ok_or(JsCodegenError::UnknownBinding { binding })?;

            Ok(Expression::new_identifier(
                SPAN,
                builder.allocator().alloc_str(name),
                builder,
            ))
        }

        Some(JsValueRepresentation::Inline) => emit_inline_expression(builder, function, value),

        Some(JsValueRepresentation::CreationAtUse) => {
            Err(JsCodegenError::UnsupportedValue { value })
        }

        Some(JsValueRepresentation::DirectEval) => Ok(Expression::new_identifier(
            SPAN,
            builder.allocator().alloc_str("eval"),
            builder,
        )),

        None => Err(JsCodegenError::UnsupportedValue { value }),
    }
}

fn emit_inline_expression<'ast>(
    builder: &AstBuilder<'ast>,
    function: &JsFunctionIr,
    value: ValueId,
) -> Result<Expression<'ast>, JsCodegenError> {
    let value_data = function
        .value(value)
        .ok_or(JsCodegenError::UnknownValue { value })?;

    let ValueDefinition::OperationResult {
        operation,
        result_index: 0,
    } = *value_data.definition()
    else {
        return Err(JsCodegenError::UnsupportedValue { value });
    };

    let operation_data = function
        .operation(operation)
        .ok_or(JsCodegenError::UnknownOperation { operation })?;

    match operation_data.kind() {
        OperationKind::Constant(constant) => {
            Ok(emit_constant_expression(builder, constant.value()))
        }

        OperationKind::LoadThis(_) => Ok(emit_load_this_expression(builder)),

        OperationKind::LoadArguments(_) => Ok(emit_load_arguments_expression(builder)),

        OperationKind::MetaProperty(meta) => Ok(emit_meta_property_expression(builder, meta)),

        _ => Err(JsCodegenError::UnsupportedValue { value }),
    }
}
