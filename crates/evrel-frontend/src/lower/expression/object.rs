//! JavaScript object-expression lowering.

use evrel_js_ir::{
    ObjectLiteralEntry, ObjectLiteralKey, ObjectLiteralOp, ObjectMethodKind, OperationKind, ValueId,
};
use oxc_ast::ast::{
    Expression, ObjectExpression, ObjectProperty, ObjectPropertyKind, PropertyKind,
};

use crate::{
    FrontendError,
    lower::{FunctionLowerer, lower_object_method_function},
};

use super::lower_expression;

/// Lowers an object literal in source evaluation order.
pub(super) fn lower_object_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &ObjectExpression<'_>,
) -> Result<ValueId, FrontendError> {
    let mut entries = Vec::with_capacity(expression.properties.len());

    for property in &expression.properties {
        match property {
            ObjectPropertyKind::SpreadProperty(spread) => {
                let expression = lowerer.build_expression_region(|lowerer| {
                    lower_expression(lowerer, &spread.argument)
                })?;

                entries.push(ObjectLiteralEntry::Spread { expression });
            }

            ObjectPropertyKind::ObjectProperty(property) => {
                entries.push(lower_object_property(lowerer, property)?);
            }
        }
    }

    Ok(lowerer.emit_value(
        OperationKind::ObjectLiteral(ObjectLiteralOp::new(entries)),
        [],
    ))
}

fn lower_object_property(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    property: &ObjectProperty<'_>,
) -> Result<ObjectLiteralEntry, FrontendError> {
    if is_prototype_setter(property) {
        let expression = lowerer
            .build_expression_region(|lowerer| lower_expression(lowerer, &property.value))?;

        return Ok(ObjectLiteralEntry::Prototype { expression });
    }

    if property.method || property.kind != PropertyKind::Init {
        return lower_object_method(lowerer, property);
    }

    let key = lower_object_key(lowerer, property)?;
    let value =
        lowerer.build_expression_region(|lowerer| lower_expression(lowerer, &property.value))?;

    Ok(ObjectLiteralEntry::Property { key, value })
}

fn lower_object_method(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    property: &ObjectProperty<'_>,
) -> Result<ObjectLiteralEntry, FrontendError> {
    let kind = match property.kind {
        PropertyKind::Init if property.method => ObjectMethodKind::Method,
        PropertyKind::Get => ObjectMethodKind::Getter,
        PropertyKind::Set => ObjectMethodKind::Setter,
        PropertyKind::Init => unreachable!("ordinary properties are lowered separately"),
    };
    let key = lower_object_key(lowerer, property)?;
    let Expression::FunctionExpression(function) = &property.value else {
        return Err(FrontendError::UnsupportedExpression);
    };
    let function = lower_object_method_function(lowerer, function)?;

    Ok(ObjectLiteralEntry::Method {
        kind,
        key,
        function,
    })
}

fn lower_object_key(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    property: &ObjectProperty<'_>,
) -> Result<ObjectLiteralKey, FrontendError> {
    if property.computed {
        let expression = property
            .key
            .as_expression()
            .ok_or(FrontendError::UnsupportedExpression)?;
        let expression =
            lowerer.build_expression_region(|lowerer| lower_expression(lowerer, expression))?;

        return Ok(ObjectLiteralKey::Computed { expression });
    }

    let name = property
        .key
        .static_name()
        .ok_or(FrontendError::UnsupportedExpression)?
        .into_owned()
        .into_boxed_str();

    Ok(ObjectLiteralKey::Static(name))
}

fn is_prototype_setter(property: &ObjectProperty<'_>) -> bool {
    property.kind == PropertyKind::Init
        && !property.method
        && !property.shorthand
        && !property.computed
        && property.key.is_specific_static_name("__proto__")
}
