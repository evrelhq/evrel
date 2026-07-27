//! JavaScript binding-pattern emission.

use evrel_ir::{BindingPattern as IrBindingPattern, FunctionIr, ModuleIr, ObjectBindingProperty};
use oxc_allocator::{Box as ArenaBox, GetAllocator, Vec as ArenaVec};
use oxc_ast::{
    AstBuilder,
    ast::{
        BindingPattern as AstBindingPattern, BindingProperty, BindingRestElement, Expression,
        FormalParameterKind, FormalParameterRest, FormalParameters, FunctionBody,
        PropertyKey as AstPropertyKey, Statement, TSTypeAnnotation, TSTypeParameterDeclaration,
        TSTypeParameterInstantiation, VariableDeclarationKind, VariableDeclarator,
    },
};
use oxc_span::SPAN;

use crate::{
    JsCodegenError,
    plan::{JsFunctionPlan, JsModulePlan},
};

use super::{
    binding::binding_name, object::emit_static_object_key, region::emit_expression_region,
};

pub(crate) fn emit_formal_parameter_pattern<'ast>(
    builder: &AstBuilder<'ast>,
    module: &ModuleIr,
    output_plan: &JsModulePlan,
    function: &FunctionIr,
    plan: &JsFunctionPlan,
    pattern: &IrBindingPattern,
) -> Result<(AstBindingPattern<'ast>, Option<Expression<'ast>>), JsCodegenError> {
    match pattern {
        IrBindingPattern::Default {
            target,
            initializer,
        } => Ok((
            emit_binding_pattern_impl(builder, module, output_plan, function, plan, target, true)?,
            Some(emit_formal_parameter_region(
                builder,
                module,
                output_plan,
                function,
                plan,
                initializer.region(),
            )?),
        )),

        _ => Ok((
            emit_binding_pattern_impl(builder, module, output_plan, function, plan, pattern, true)?,
            None,
        )),
    }
}

fn emit_formal_parameter_region<'ast>(
    builder: &AstBuilder<'ast>,
    module: &ModuleIr,
    output_plan: &JsModulePlan,
    function: &FunctionIr,
    plan: &JsFunctionPlan,
    region: evrel_ir::RegionId,
) -> Result<Expression<'ast>, JsCodegenError> {
    let expression = emit_expression_region(builder, module, output_plan, function, plan, region)?;
    let mut locals = Vec::new();
    collect_parameter_region_locals(function, plan, region, &mut locals)?;

    if locals.is_empty() {
        return Ok(expression);
    }

    let mut declarators = ArenaVec::with_capacity_in(locals.len(), builder);
    for local in locals {
        let name = plan
            .local_name(local)
            .expect("every parameter-region local must receive a name");
        declarators.push(VariableDeclarator::new(
            SPAN,
            VariableDeclarationKind::Let,
            AstBindingPattern::new_binding_identifier(
                SPAN,
                builder.allocator().alloc_str(name),
                builder,
            ),
            None::<ArenaBox<'ast, TSTypeAnnotation<'ast>>>,
            None,
            false,
            builder,
        ));
    }

    let statements = ArenaVec::from_array_in(
        [
            Statement::new_variable_declaration(
                SPAN,
                VariableDeclarationKind::Let,
                declarators,
                false,
                builder,
            ),
            Statement::new_return_statement(SPAN, Some(expression), builder),
        ],
        builder,
    );
    let parameters = FormalParameters::boxed(
        SPAN,
        FormalParameterKind::ArrowFormalParameters,
        ArenaVec::new_in(builder),
        None::<ArenaBox<'ast, FormalParameterRest<'ast>>>,
        builder,
    );
    let body = FunctionBody::boxed(SPAN, ArenaVec::new_in(builder), statements, builder);
    let arrow = Expression::new_arrow_function_expression(
        SPAN,
        false,
        false,
        None::<ArenaBox<'ast, TSTypeParameterDeclaration<'ast>>>,
        parameters,
        None::<ArenaBox<'ast, TSTypeAnnotation<'ast>>>,
        body,
        builder,
    );

    Ok(Expression::new_call_expression(
        SPAN,
        arrow,
        None::<ArenaBox<'ast, TSTypeParameterInstantiation<'ast>>>,
        ArenaVec::new_in(builder),
        false,
        builder,
    ))
}

fn collect_parameter_region_locals(
    function: &FunctionIr,
    plan: &JsFunctionPlan,
    region: evrel_ir::RegionId,
    locals: &mut Vec<crate::plan::JsLocalId>,
) -> Result<(), JsCodegenError> {
    let region_data = function
        .region(region)
        .ok_or(JsCodegenError::UnknownRegion { region })?;

    for &block in region_data.blocks() {
        let block = function
            .block(block)
            .ok_or(JsCodegenError::UnsupportedExpressionRegion { region })?;

        for &operation in block.operations() {
            let operation = function
                .operation(operation)
                .ok_or(JsCodegenError::UnsupportedExpressionRegion { region })?;

            for &result in operation.results() {
                if let Some(crate::plan::JsValueRepresentation::Temporary(local)) =
                    plan.value(result)
                    && plan.local_name(local).is_some()
                    && !locals.contains(&local)
                {
                    locals.push(local);
                }
            }

            for child in operation.regions() {
                collect_parameter_region_locals(function, plan, child, locals)?;
            }
        }
    }

    Ok(())
}

pub(crate) fn emit_binding_pattern<'ast>(
    builder: &AstBuilder<'ast>,
    module: &ModuleIr,
    output_plan: &JsModulePlan,
    function: &FunctionIr,
    plan: &JsFunctionPlan,
    pattern: &IrBindingPattern,
) -> Result<AstBindingPattern<'ast>, JsCodegenError> {
    emit_binding_pattern_impl(builder, module, output_plan, function, plan, pattern, false)
}

fn emit_binding_pattern_impl<'ast>(
    builder: &AstBuilder<'ast>,
    module: &ModuleIr,
    output_plan: &JsModulePlan,
    function: &FunctionIr,
    plan: &JsFunctionPlan,
    pattern: &IrBindingPattern,
    formal_parameter: bool,
) -> Result<AstBindingPattern<'ast>, JsCodegenError> {
    match pattern {
        IrBindingPattern::Binding { binding } => {
            let name = binding_name(plan, *binding)?;

            Ok(AstBindingPattern::new_binding_identifier(
                SPAN,
                builder.allocator().alloc_str(name),
                builder,
            ))
        }

        IrBindingPattern::Array { elements, rest } => {
            let mut emitted = ArenaVec::with_capacity_in(elements.len(), builder);

            for element in elements {
                emitted.push(
                    element
                        .as_ref()
                        .map(|element| {
                            emit_binding_pattern_impl(
                                builder,
                                module,
                                output_plan,
                                function,
                                plan,
                                element,
                                formal_parameter,
                            )
                        })
                        .transpose()?,
                );
            }

            let rest = rest
                .as_deref()
                .map(|rest| {
                    emit_binding_rest(
                        builder,
                        module,
                        output_plan,
                        function,
                        plan,
                        rest,
                        formal_parameter,
                    )
                })
                .transpose()?;

            Ok(AstBindingPattern::new_array_pattern(
                SPAN, emitted, rest, builder,
            ))
        }

        IrBindingPattern::Object { properties, rest } => {
            let mut emitted = ArenaVec::with_capacity_in(properties.len(), builder);

            for property in properties {
                let (key, computed, target) = match property {
                    ObjectBindingProperty::Static { name, target } => {
                        (emit_static_object_key(builder, name), false, target)
                    }

                    ObjectBindingProperty::Computed { key, target } => (
                        AstPropertyKey::from(emit_pattern_region(
                            builder,
                            module,
                            output_plan,
                            function,
                            plan,
                            key.region(),
                            formal_parameter,
                        )?),
                        true,
                        target,
                    ),
                };

                emitted.push(BindingProperty::new(
                    SPAN,
                    key,
                    emit_binding_pattern_impl(
                        builder,
                        module,
                        output_plan,
                        function,
                        plan,
                        target,
                        formal_parameter,
                    )?,
                    false,
                    computed,
                    builder,
                ));
            }

            let rest = rest
                .as_deref()
                .map(|rest| {
                    emit_binding_rest(
                        builder,
                        module,
                        output_plan,
                        function,
                        plan,
                        rest,
                        formal_parameter,
                    )
                })
                .transpose()?;

            Ok(AstBindingPattern::new_object_pattern(
                SPAN, emitted, rest, builder,
            ))
        }

        IrBindingPattern::Default {
            target,
            initializer,
        } => Ok(AstBindingPattern::new_assignment_pattern(
            SPAN,
            emit_binding_pattern_impl(
                builder,
                module,
                output_plan,
                function,
                plan,
                target,
                formal_parameter,
            )?,
            emit_pattern_region(
                builder,
                module,
                output_plan,
                function,
                plan,
                initializer.region(),
                formal_parameter,
            )?,
            builder,
        )),
    }
}

fn emit_pattern_region<'ast>(
    builder: &AstBuilder<'ast>,
    module: &ModuleIr,
    output_plan: &JsModulePlan,
    function: &FunctionIr,
    plan: &JsFunctionPlan,
    region: evrel_ir::RegionId,
    formal_parameter: bool,
) -> Result<Expression<'ast>, JsCodegenError> {
    if formal_parameter {
        emit_formal_parameter_region(builder, module, output_plan, function, plan, region)
    } else {
        emit_expression_region(builder, module, output_plan, function, plan, region)
    }
}

fn emit_binding_rest<'ast>(
    builder: &AstBuilder<'ast>,
    module: &ModuleIr,
    output_plan: &JsModulePlan,
    function: &FunctionIr,
    plan: &JsFunctionPlan,
    pattern: &IrBindingPattern,
    formal_parameter: bool,
) -> Result<ArenaBox<'ast, BindingRestElement<'ast>>, JsCodegenError> {
    Ok(BindingRestElement::boxed(
        SPAN,
        emit_binding_pattern_impl(
            builder,
            module,
            output_plan,
            function,
            plan,
            pattern,
            formal_parameter,
        )?,
        builder,
    ))
}
