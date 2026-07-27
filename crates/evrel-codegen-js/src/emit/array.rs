//! JavaScript array-literal emission.

use evrel_ir::{ArrayLiteralElement, ArrayLiteralOp, FunctionIr, ModuleIr};
use oxc_allocator::Vec as ArenaVec;
use oxc_ast::{
    AstBuilder,
    ast::{ArrayExpressionElement, Expression},
};
use oxc_span::SPAN;

use crate::{
    JsCodegenError,
    plan::{JsFunctionPlan, JsModulePlan},
};

use super::region::emit_expression_region;

pub(crate) fn emit_array_literal_expression<'ast>(
    builder: &AstBuilder<'ast>,
    module: &ModuleIr,
    output_plan: &JsModulePlan,
    function: &FunctionIr,
    plan: &JsFunctionPlan,
    array: &ArrayLiteralOp,
) -> Result<Expression<'ast>, JsCodegenError> {
    let mut elements = ArenaVec::with_capacity_in(array.elements().len(), builder);

    for element in array.elements() {
        elements.push(match element {
            ArrayLiteralElement::Value { expression } => ArrayExpressionElement::from(
                emit_expression_region(builder, module, output_plan, function, plan, *expression)?,
            ),
            ArrayLiteralElement::Spread { expression } => {
                ArrayExpressionElement::new_spread_element(
                    SPAN,
                    emit_expression_region(
                        builder,
                        module,
                        output_plan,
                        function,
                        plan,
                        *expression,
                    )?,
                    builder,
                )
            }
            ArrayLiteralElement::Elision => ArrayExpressionElement::new_elision(SPAN, builder),
        });
    }

    Ok(Expression::new_array_expression(SPAN, elements, builder))
}
