//! Operand emission from planned value representations.

use evrel_js_ir::{JsFunctionIr, JsModuleIr, OperationKind, ValueDefinition, ValueId};
use oxc_ast::{AstBuilder, ast::Expression};

use crate::{
    JsCodegenError,
    js::plan::{JsFunctionPlan, JsModulePlan, JsValueRepresentation},
};

use super::{
    class::emit_class_expression, function::emit_create_function_expression,
    value::emit_value_expression,
};

pub(super) fn emit_operand_expression<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    output_plan: &JsModulePlan,
    function: &JsFunctionIr,
    plan: &JsFunctionPlan,
    value: ValueId,
) -> Result<Expression<'ast>, JsCodegenError> {
    if plan.value(value) != Some(JsValueRepresentation::CreationAtUse) {
        return emit_value_expression(builder, function, plan, value);
    }

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
    let expression = match operation_data.kind() {
        OperationKind::CreateFunction(create) => {
            emit_create_function_expression(builder, module, output_plan, operation, create)?
        }
        OperationKind::CreateClass(class) => {
            emit_class_expression(builder, module, output_plan, function, plan, class)?
        }
        _ => return Err(JsCodegenError::UnsupportedValue { value }),
    };

    Ok(expression)
}
