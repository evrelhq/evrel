//! JavaScript binding-destructuring emission.

use evrel_ir::{
    AssignmentPattern as IrAssignmentPattern, AssignmentTarget as IrAssignmentTarget, BindingKind,
    BindingPattern, BindingWriteMode, DestructureAssignmentOp, DestructureBindingOp, FunctionIr,
    ModuleIr, ObjectAssignmentProperty, OperationId, ValueId,
};
use oxc_allocator::{Box as ArenaBox, GetAllocator, Vec as ArenaVec};
use oxc_ast::{
    AstBuilder,
    ast::{
        AssignmentOperator, AssignmentTarget, AssignmentTargetMaybeDefault,
        AssignmentTargetProperty, AssignmentTargetRest, Expression, PropertyKey, Statement,
        TSTypeAnnotation, VariableDeclarationKind, VariableDeclarator,
    },
};
use oxc_span::SPAN;

use crate::{
    JsCodegenError,
    plan::{JsFunctionPlan, JsModulePlan},
};

use super::{
    FunctionEmission,
    binding::binding_name,
    object::emit_static_object_key,
    pattern::emit_binding_pattern,
    property::{
        emit_computed_member_expression, emit_private_member_expression,
        emit_static_member_expression,
    },
    region::emit_expression_region,
    value::emit_value_expression,
};

pub(crate) fn emit_destructure_binding_statement<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    operation: OperationId,
    destructure: &DestructureBindingOp,
    operands: &[ValueId],
) -> Result<Statement<'ast>, JsCodegenError> {
    let builder = emission.builder;
    let module = emission.module;
    let output_plan = emission.output_plan;
    let function = emission.function;
    let plan = emission.plan;

    let [source] = operands else {
        return Err(JsCodegenError::MalformedOperation { operation });
    };

    let [] = function
        .operation(operation)
        .ok_or(JsCodegenError::UnknownOperation { operation })?
        .results()
    else {
        return Err(JsCodegenError::MalformedOperation { operation });
    };

    let kind = binding_pattern_declaration_kind(
        module,
        operation,
        destructure.pattern(),
        destructure.mode(),
    )?;
    let initializer = emit_value_expression(builder, function, plan, *source)?;
    let declarator = VariableDeclarator::new(
        SPAN,
        kind,
        emit_binding_pattern(
            builder,
            module,
            output_plan,
            function,
            plan,
            destructure.pattern(),
        )?,
        None::<ArenaBox<'ast, TSTypeAnnotation<'ast>>>,
        Some(initializer),
        false,
        builder,
    );

    Ok(Statement::new_variable_declaration(
        SPAN,
        kind,
        ArenaVec::from_array_in([declarator], builder),
        false,
        builder,
    ))
}

pub(crate) fn emit_destructure_assignment_statement<'ast>(
    emission: FunctionEmission<'_, 'ast>,
    operation: OperationId,
    destructure: &DestructureAssignmentOp,
    operands: &[ValueId],
) -> Result<Statement<'ast>, JsCodegenError> {
    let builder = emission.builder;
    let module = emission.module;
    let output_plan = emission.output_plan;
    let function = emission.function;
    let plan = emission.plan;

    let [source] = operands else {
        return Err(JsCodegenError::MalformedOperation { operation });
    };
    let operation_data = function
        .operation(operation)
        .ok_or(JsCodegenError::UnknownOperation { operation })?;
    let [] = operation_data.results() else {
        return Err(JsCodegenError::MalformedOperation { operation });
    };

    let assignment = Expression::new_assignment_expression(
        SPAN,
        AssignmentOperator::Assign,
        emit_assignment_pattern(
            builder,
            module,
            output_plan,
            function,
            plan,
            operation,
            destructure.pattern(),
        )?,
        emit_value_expression(builder, function, plan, *source)?,
        builder,
    );

    Ok(Statement::new_expression_statement(
        SPAN, assignment, builder,
    ))
}

fn emit_assignment_pattern<'ast>(
    builder: &AstBuilder<'ast>,
    module: &ModuleIr,
    output_plan: &JsModulePlan,
    function: &FunctionIr,
    plan: &JsFunctionPlan,
    operation: OperationId,
    pattern: &IrAssignmentPattern,
) -> Result<AssignmentTarget<'ast>, JsCodegenError> {
    match pattern {
        IrAssignmentPattern::Target { target } => {
            emit_assignment_target(builder, module, output_plan, function, plan, target)
        }

        IrAssignmentPattern::Array { elements, rest } => {
            let mut emitted = ArenaVec::with_capacity_in(elements.len(), builder);

            for element in elements {
                emitted.push(
                    element
                        .as_ref()
                        .map(|pattern| {
                            emit_assignment_pattern_maybe_default(
                                builder,
                                module,
                                output_plan,
                                function,
                                plan,
                                operation,
                                pattern,
                            )
                        })
                        .transpose()?,
                );
            }

            let rest = rest
                .as_deref()
                .map(|pattern| {
                    emit_assignment_rest(
                        builder,
                        module,
                        output_plan,
                        function,
                        plan,
                        operation,
                        pattern,
                    )
                })
                .transpose()?;

            Ok(AssignmentTarget::new_array_assignment_target(
                SPAN, emitted, rest, builder,
            ))
        }

        IrAssignmentPattern::Object { properties, rest } => {
            let mut emitted = ArenaVec::with_capacity_in(properties.len(), builder);

            for property in properties {
                let (key, computed, target) = match property {
                    ObjectAssignmentProperty::Static { name, target } => {
                        (emit_static_object_key(builder, name), false, target)
                    }

                    ObjectAssignmentProperty::Computed { key, target } => (
                        PropertyKey::from(emit_expression_region(
                            builder,
                            module,
                            output_plan,
                            function,
                            plan,
                            key.region(),
                        )?),
                        true,
                        target,
                    ),
                };

                emitted.push(
                    AssignmentTargetProperty::new_assignment_target_property_property(
                        SPAN,
                        key,
                        emit_assignment_pattern_maybe_default(
                            builder,
                            module,
                            output_plan,
                            function,
                            plan,
                            operation,
                            target,
                        )?,
                        computed,
                        builder,
                    ),
                );
            }

            let rest = rest
                .as_deref()
                .map(|pattern| {
                    emit_assignment_rest(
                        builder,
                        module,
                        output_plan,
                        function,
                        plan,
                        operation,
                        pattern,
                    )
                })
                .transpose()?;

            Ok(AssignmentTarget::new_object_assignment_target(
                SPAN, emitted, rest, builder,
            ))
        }

        IrAssignmentPattern::Default { .. } => {
            Err(JsCodegenError::MalformedOperation { operation })
        }
    }
}

fn emit_assignment_pattern_maybe_default<'ast>(
    builder: &AstBuilder<'ast>,
    module: &ModuleIr,
    output_plan: &JsModulePlan,
    function: &FunctionIr,
    plan: &JsFunctionPlan,
    operation: OperationId,
    pattern: &IrAssignmentPattern,
) -> Result<AssignmentTargetMaybeDefault<'ast>, JsCodegenError> {
    match pattern {
        IrAssignmentPattern::Default {
            target,
            initializer,
        } => Ok(
            AssignmentTargetMaybeDefault::new_assignment_target_with_default(
                SPAN,
                emit_assignment_pattern(
                    builder,
                    module,
                    output_plan,
                    function,
                    plan,
                    operation,
                    target,
                )?,
                emit_expression_region(
                    builder,
                    module,
                    output_plan,
                    function,
                    plan,
                    initializer.region(),
                )?,
                builder,
            ),
        ),

        _ => Ok(AssignmentTargetMaybeDefault::from(emit_assignment_pattern(
            builder,
            module,
            output_plan,
            function,
            plan,
            operation,
            pattern,
        )?)),
    }
}

fn emit_assignment_rest<'ast>(
    builder: &AstBuilder<'ast>,
    module: &ModuleIr,
    output_plan: &JsModulePlan,
    function: &FunctionIr,
    plan: &JsFunctionPlan,
    operation: OperationId,
    pattern: &IrAssignmentPattern,
) -> Result<ArenaBox<'ast, AssignmentTargetRest<'ast>>, JsCodegenError> {
    Ok(AssignmentTargetRest::boxed(
        SPAN,
        emit_assignment_pattern(
            builder,
            module,
            output_plan,
            function,
            plan,
            operation,
            pattern,
        )?,
        builder,
    ))
}

fn emit_assignment_target<'ast>(
    builder: &AstBuilder<'ast>,
    module: &ModuleIr,
    output_plan: &JsModulePlan,
    function: &FunctionIr,
    plan: &JsFunctionPlan,
    target: &IrAssignmentTarget,
) -> Result<AssignmentTarget<'ast>, JsCodegenError> {
    match target {
        IrAssignmentTarget::Binding { binding } => {
            Ok(AssignmentTarget::new_assignment_target_identifier(
                SPAN,
                builder.allocator().alloc_str(binding_name(plan, *binding)?),
                builder,
            ))
        }

        IrAssignmentTarget::Global { name } => {
            Ok(AssignmentTarget::new_assignment_target_identifier(
                SPAN,
                builder.allocator().alloc_str(name),
                builder,
            ))
        }

        IrAssignmentTarget::StaticProperty { object, name } => {
            Ok(AssignmentTarget::from(emit_static_member_expression(
                builder,
                emit_expression_region(
                    builder,
                    module,
                    output_plan,
                    function,
                    plan,
                    object.region(),
                )?,
                name,
            )))
        }

        IrAssignmentTarget::ComputedProperty { object, key } => {
            Ok(AssignmentTarget::from(emit_computed_member_expression(
                builder,
                emit_expression_region(
                    builder,
                    module,
                    output_plan,
                    function,
                    plan,
                    object.region(),
                )?,
                emit_expression_region(builder, module, output_plan, function, plan, key.region())?,
            )))
        }

        IrAssignmentTarget::PrivateProperty {
            object,
            private_name,
        } => Ok(AssignmentTarget::from(emit_private_member_expression(
            builder,
            module,
            emit_expression_region(
                builder,
                module,
                output_plan,
                function,
                plan,
                object.region(),
            )?,
            *private_name,
        )?)),

        IrAssignmentTarget::StaticSuperProperty { name } => Ok(AssignmentTarget::from(
            emit_static_member_expression(builder, Expression::new_super(SPAN, builder), name),
        )),

        IrAssignmentTarget::ComputedSuperProperty { key } => {
            Ok(AssignmentTarget::from(emit_computed_member_expression(
                builder,
                Expression::new_super(SPAN, builder),
                emit_expression_region(builder, module, output_plan, function, plan, key.region())?,
            )))
        }
    }
}

fn binding_pattern_declaration_kind(
    module: &ModuleIr,
    operation: OperationId,
    pattern: &BindingPattern,
    mode: BindingWriteMode,
) -> Result<VariableDeclarationKind, JsCodegenError> {
    let expected = match mode {
        BindingWriteMode::Store => VariableDeclarationKind::Var,

        BindingWriteMode::Initialize => {
            let Some(binding) = pattern.binding_ids().first().copied() else {
                return Ok(VariableDeclarationKind::Let);
            };

            match module
                .binding(binding)
                .ok_or(JsCodegenError::UnknownBinding { binding })?
                .kind()
            {
                BindingKind::Const => VariableDeclarationKind::Const,
                BindingKind::Let | BindingKind::Catch => VariableDeclarationKind::Let,

                _ => {
                    return Err(JsCodegenError::UnsupportedOperation {
                        operation,
                        reason: concat!(file!(), ":", line!()),
                    });
                }
            }
        }
    };

    for binding in pattern.binding_ids() {
        let actual = module
            .binding(binding)
            .ok_or(JsCodegenError::UnknownBinding { binding })?
            .kind();
        let compatible = match expected {
            VariableDeclarationKind::Var => actual == BindingKind::Var,
            VariableDeclarationKind::Const => actual == BindingKind::Const,
            VariableDeclarationKind::Let => {
                matches!(actual, BindingKind::Let | BindingKind::Catch)
            }
            _ => false,
        };

        if !compatible {
            return Err(JsCodegenError::UnsupportedOperation {
                operation,
                reason: concat!(file!(), ":", line!()),
            });
        }
    }

    Ok(expected)
}
