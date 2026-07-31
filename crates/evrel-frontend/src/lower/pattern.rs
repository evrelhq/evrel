//! Oxc binding-pattern conversion.

use evrel_js_ir::{
    AssignmentPattern as IrAssignmentPattern, AssignmentTarget as IrAssignmentTarget, BindingKind,
    BindingPattern as IrBindingPattern, BindingWriteMode, DestructureAssignmentOp,
    DestructureBindingOp, InitializeBindingOp, ObjectAssignmentProperty, ObjectBindingProperty,
    OperationKind, PatternExpression, StoreBindingOp, ValueId,
};
use oxc_ast::ast::{
    AssignmentTarget as OxcAssignmentTarget, AssignmentTargetMaybeDefault,
    AssignmentTargetProperty, BindingPattern as OxcBindingPattern, Expression, IdentifierReference,
    ObjectAssignmentTarget, SimpleAssignmentTarget,
};

use crate::{
    FrontendError,
    lower::{FunctionLowerer, expression::lower_expression},
};

/// Creates Evrel bindings for every identifier declared by a pattern.
pub(super) fn declare_pattern_bindings(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    pattern: &OxcBindingPattern<'_>,
    kind: BindingKind,
) {
    match pattern {
        OxcBindingPattern::BindingIdentifier(identifier) => {
            let symbol = identifier.symbol_id();

            if !lowerer.contains_binding(symbol) {
                lowerer.declare_binding(symbol, identifier.name.as_str(), kind);
            }
        }

        OxcBindingPattern::ArrayPattern(array) => {
            for element in array.elements.iter().flatten() {
                declare_pattern_bindings(lowerer, element, kind);
            }

            if let Some(rest) = &array.rest {
                declare_pattern_bindings(lowerer, &rest.argument, kind);
            }
        }

        OxcBindingPattern::ObjectPattern(object) => {
            for property in &object.properties {
                declare_pattern_bindings(lowerer, &property.value, kind);
            }

            if let Some(rest) = &object.rest {
                declare_pattern_bindings(lowerer, &rest.argument, kind);
            }
        }

        OxcBindingPattern::AssignmentPattern(assignment) => {
            declare_pattern_bindings(lowerer, &assignment.left, kind);
        }
    }
}

/// Converts an Oxc binding pattern into compiler-owned IR.
///
/// Pattern expressions are lowered into inline regions so destructuring retains
/// control over their exact evaluation time.
pub(super) fn lower_binding_pattern(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    pattern: &OxcBindingPattern<'_>,
) -> Result<IrBindingPattern, FrontendError> {
    match pattern {
        OxcBindingPattern::BindingIdentifier(identifier) => Ok(IrBindingPattern::binding(
            lowerer.binding_for_symbol(identifier.symbol_id()),
        )),

        OxcBindingPattern::ArrayPattern(array) => {
            let mut elements = Vec::with_capacity(array.elements.len());
            for element in &array.elements {
                elements.push(
                    element
                        .as_ref()
                        .map(|element| lower_binding_pattern(lowerer, element))
                        .transpose()?,
                );
            }

            let rest = match &array.rest {
                Some(rest) => Some(lower_binding_pattern(lowerer, &rest.argument)?),
                None => None,
            };

            Ok(IrBindingPattern::array(elements, rest))
        }

        OxcBindingPattern::ObjectPattern(object) => {
            let mut properties = Vec::with_capacity(object.properties.len());
            for property in &object.properties {
                let key = if property.computed {
                    let expression = property
                        .key
                        .as_expression()
                        .ok_or(FrontendError::InvalidBindingPattern)?;
                    let region = lowerer
                        .build_expression_region(|lowerer| lower_expression(lowerer, expression))?;

                    Some(PatternExpression::new(region))
                } else {
                    None
                };
                let target = lower_binding_pattern(lowerer, &property.value)?;

                properties.push(match key {
                    Some(key) => ObjectBindingProperty::computed_property(key, target),
                    None => {
                        let name = property
                            .key
                            .static_name()
                            .ok_or(FrontendError::InvalidBindingPattern)?
                            .into_owned()
                            .into_boxed_str();

                        ObjectBindingProperty::static_property(name, target)
                    }
                });
            }

            let rest = match &object.rest {
                Some(rest) => Some(lower_binding_pattern(lowerer, &rest.argument)?),
                None => None,
            };

            Ok(IrBindingPattern::object(properties, rest))
        }

        OxcBindingPattern::AssignmentPattern(assignment) => {
            let target = lower_binding_pattern(lowerer, &assignment.left)?;
            let initializer = lowerer
                .build_expression_region(|lowerer| lower_expression(lowerer, &assignment.right))?;

            Ok(IrBindingPattern::default(
                target,
                PatternExpression::new(initializer),
            ))
        }
    }
}

/// Converts an Oxc assignment target into a compiler-owned IR pattern.
///
/// Expressions that construct assignment references are lowered into inline
/// regions so they execute only when destructuring reaches their target.
pub(super) fn lower_assignment_pattern(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    target: &OxcAssignmentTarget<'_>,
) -> Result<IrAssignmentPattern, FrontendError> {
    match target {
        OxcAssignmentTarget::ArrayAssignmentTarget(array) => {
            let mut elements = Vec::with_capacity(array.elements.len());

            for element in &array.elements {
                elements.push(
                    element
                        .as_ref()
                        .map(|element| lower_assignment_pattern_maybe_default(lowerer, element))
                        .transpose()?,
                );
            }

            let rest = array
                .rest
                .as_ref()
                .map(|rest| lower_assignment_pattern(lowerer, &rest.target))
                .transpose()?;

            Ok(IrAssignmentPattern::array(elements, rest))
        }

        OxcAssignmentTarget::ObjectAssignmentTarget(object) => {
            lower_object_assignment_pattern(lowerer, object)
        }

        _ => {
            let target = target
                .as_simple_assignment_target()
                .ok_or(FrontendError::UnsupportedAssignmentTarget)?;

            Ok(IrAssignmentPattern::target(lower_simple_assignment_target(
                lowerer, target,
            )?))
        }
    }
}

fn lower_assignment_pattern_maybe_default(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    target: &AssignmentTargetMaybeDefault<'_>,
) -> Result<IrAssignmentPattern, FrontendError> {
    if let AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(default) = target {
        let initializer = if default.binding.as_assignment_target_pattern().is_some() {
            Some(
                lowerer
                    .build_expression_region(|lowerer| lower_expression(lowerer, &default.init))?,
            )
        } else {
            None
        };
        let target = lower_assignment_pattern(lowerer, &default.binding)?;
        let initializer = match initializer {
            Some(initializer) => initializer,
            None => lowerer
                .build_expression_region(|lowerer| lower_expression(lowerer, &default.init))?,
        };

        return Ok(IrAssignmentPattern::default(
            target,
            PatternExpression::new(initializer),
        ));
    }

    let target = target
        .as_assignment_target()
        .ok_or(FrontendError::UnsupportedAssignmentTarget)?;

    lower_assignment_pattern(lowerer, target)
}

fn lower_object_assignment_pattern(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    object: &ObjectAssignmentTarget<'_>,
) -> Result<IrAssignmentPattern, FrontendError> {
    let mut properties = Vec::with_capacity(object.properties.len());

    for property in &object.properties {
        match property {
            AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(property) => {
                let mut target = IrAssignmentPattern::target(lower_identifier_assignment_target(
                    lowerer,
                    &property.binding,
                ));

                if let Some(initializer) = &property.init {
                    let initializer = lowerer.build_expression_region(|lowerer| {
                        lower_expression(lowerer, initializer)
                    })?;
                    target =
                        IrAssignmentPattern::default(target, PatternExpression::new(initializer));
                }

                properties.push(ObjectAssignmentProperty::static_property(
                    property.binding.name.as_str(),
                    target,
                ));
            }

            AssignmentTargetProperty::AssignmentTargetPropertyProperty(property) => {
                if property.computed {
                    let expression = property
                        .name
                        .as_expression()
                        .ok_or(FrontendError::UnsupportedAssignmentTarget)?;
                    let key = lowerer
                        .build_expression_region(|lowerer| lower_expression(lowerer, expression))?;
                    let target =
                        lower_assignment_pattern_maybe_default(lowerer, &property.binding)?;

                    properties.push(ObjectAssignmentProperty::computed_property(
                        PatternExpression::new(key),
                        target,
                    ));
                } else {
                    let target =
                        lower_assignment_pattern_maybe_default(lowerer, &property.binding)?;
                    let name = property
                        .name
                        .static_name()
                        .ok_or(FrontendError::UnsupportedAssignmentTarget)?
                        .into_owned()
                        .into_boxed_str();

                    properties.push(ObjectAssignmentProperty::static_property(name, target));
                }
            }
        }
    }

    let rest = object
        .rest
        .as_ref()
        .map(|rest| lower_assignment_pattern(lowerer, &rest.target))
        .transpose()?;

    Ok(IrAssignmentPattern::object(properties, rest))
}

fn lower_simple_assignment_target(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    target: &SimpleAssignmentTarget<'_>,
) -> Result<IrAssignmentTarget, FrontendError> {
    match target {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            Ok(lower_identifier_assignment_target(lowerer, identifier))
        }

        SimpleAssignmentTarget::StaticMemberExpression(member) => {
            if member.optional {
                return Err(FrontendError::InvalidOptionalChain);
            }

            let name = member.property.name.as_str().into();

            if matches!(&member.object, Expression::Super(_)) {
                return Ok(IrAssignmentTarget::StaticSuperProperty { name });
            }

            let object = lowerer
                .build_expression_region(|lowerer| lower_expression(lowerer, &member.object))?;

            Ok(IrAssignmentTarget::StaticProperty {
                object: PatternExpression::new(object),
                name,
            })
        }

        SimpleAssignmentTarget::ComputedMemberExpression(member) => {
            if member.optional {
                return Err(FrontendError::InvalidOptionalChain);
            }

            if matches!(&member.object, Expression::Super(_)) {
                let key = lowerer.build_expression_region(|lowerer| {
                    lower_expression(lowerer, &member.expression)
                })?;

                return Ok(IrAssignmentTarget::ComputedSuperProperty {
                    key: PatternExpression::new(key),
                });
            }

            let object = lowerer
                .build_expression_region(|lowerer| lower_expression(lowerer, &member.object))?;
            let key = lowerer
                .build_expression_region(|lowerer| lower_expression(lowerer, &member.expression))?;

            Ok(IrAssignmentTarget::ComputedProperty {
                object: PatternExpression::new(object),
                key: PatternExpression::new(key),
            })
        }

        SimpleAssignmentTarget::PrivateFieldExpression(member) => {
            if member.optional {
                return Err(FrontendError::InvalidOptionalChain);
            }

            let object = lowerer
                .build_expression_region(|lowerer| lower_expression(lowerer, &member.object))?;
            let private_name = lowerer.private_name(member.field.name.as_str());

            Ok(IrAssignmentTarget::PrivateProperty {
                object: PatternExpression::new(object),
                private_name,
            })
        }

        _ => Err(FrontendError::UnsupportedAssignmentTarget),
    }
}

fn lower_identifier_assignment_target(
    lowerer: &FunctionLowerer<'_, '_, '_>,
    identifier: &IdentifierReference<'_>,
) -> IrAssignmentTarget {
    match lowerer.binding_for_reference(identifier) {
        Some(binding) => IrAssignmentTarget::Binding { binding },
        None => IrAssignmentTarget::Global {
            name: identifier.name.as_str().into(),
        },
    }
}

/// Writes one value through a compiler-owned assignment pattern.
pub(super) fn emit_assignment_pattern_write(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    pattern: IrAssignmentPattern,
    value: ValueId,
) {
    lowerer.emit(
        OperationKind::DestructureAssignment(DestructureAssignmentOp::new(pattern)),
        [value],
    );
}

/// Writes one value through a compiler-owned binding pattern.
pub(super) fn emit_binding_pattern_write(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    pattern: IrBindingPattern,
    mode: BindingWriteMode,
    value: ValueId,
) {
    let operation = if let Some(binding) = pattern.as_binding() {
        match mode {
            BindingWriteMode::Initialize => {
                OperationKind::InitializeBinding(InitializeBindingOp::new(binding))
            }
            BindingWriteMode::Store => OperationKind::StoreBinding(StoreBindingOp::new(binding)),
        }
    } else {
        OperationKind::DestructureBinding(DestructureBindingOp::new(pattern, mode))
    };

    lowerer.emit(operation, [value]);
}
