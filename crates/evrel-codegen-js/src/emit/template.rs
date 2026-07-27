//! JavaScript template-literal emission.

use evrel_ir::{OperationId, TaggedTemplateOp, TemplateLiteralOp, TemplateQuasi, ValueId};
use oxc_allocator::{Box as ArenaBox, GetAllocator, Vec as ArenaVec};
use oxc_ast::ast::{
    Expression, TSTypeParameterInstantiation, TemplateElement, TemplateElementValue,
    TemplateLiteral,
};
use oxc_span::SPAN;

use crate::JsCodegenError;

use super::{FunctionEmission, call::emit_call_target, region::emit_expression_region};

pub(crate) fn emit_template_literal_expression<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    template: &TemplateLiteralOp,
) -> Result<Expression<'ast>, JsCodegenError> {
    let builder = emission.builder;
    let (quasis, expressions) =
        emit_template_parts(emission, template.quasis(), template.substitutions())?;

    Ok(Expression::new_template_literal(
        SPAN,
        quasis,
        expressions,
        builder,
    ))
}

pub(crate) fn emit_tagged_template_expression<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    operation: OperationId,
    template: &TaggedTemplateOp,
    operands: &[ValueId],
) -> Result<Expression<'ast>, JsCodegenError> {
    let builder = emission.builder;
    let tag = emit_call_target(emission, operation, template.target(), operands)?;
    let (quasis, expressions) =
        emit_template_parts(emission, template.quasis(), template.substitutions())?;
    let quasi = TemplateLiteral::new(SPAN, quasis, expressions, builder);

    Ok(Expression::new_tagged_template_expression(
        SPAN,
        tag,
        None::<ArenaBox<'ast, TSTypeParameterInstantiation<'ast>>>,
        quasi,
        builder,
    ))
}

fn emit_template_parts<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    template_quasis: &[TemplateQuasi],
    substitutions: &[evrel_ir::RegionId],
) -> Result<
    (
        ArenaVec<'ast, TemplateElement<'ast>>,
        ArenaVec<'ast, Expression<'ast>>,
    ),
    JsCodegenError,
> {
    let builder = emission.builder;
    let mut quasis = ArenaVec::with_capacity_in(template_quasis.len(), builder);

    for (index, quasi) in template_quasis.iter().enumerate() {
        let cooked = quasi.cooked();

        quasis.push(TemplateElement::new_with_lone_surrogates(
            SPAN,
            TemplateElementValue {
                raw: builder.allocator().alloc_str(quasi.raw()).into(),
                cooked: cooked.map(|cooked| builder.allocator().alloc_str(cooked.as_str()).into()),
            },
            index + 1 == template_quasis.len(),
            cooked.is_some_and(|cooked| cooked.has_lone_surrogates()),
            builder,
        ));
    }

    let mut expressions = ArenaVec::with_capacity_in(substitutions.len(), builder);

    for substitution in substitutions {
        expressions.push(emit_expression_region(
            builder,
            emission.module,
            emission.output_plan,
            emission.function,
            emission.plan,
            *substitution,
        )?);
    }

    Ok((quasis, expressions))
}
