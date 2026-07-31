//! JavaScript object-literal emission.

use evrel_js_ir::{
    FunctionKind, JsFunctionIr, JsModuleIr, ObjectLiteralEntry, ObjectLiteralKey, ObjectLiteralOp,
    ObjectMethodKind,
};
use oxc_allocator::{GetAllocator, Vec as ArenaVec};
use oxc_ast::{
    AstBuilder,
    ast::{Expression, ObjectPropertyKind, PropertyKey as AstPropertyKey, PropertyKind},
};
use oxc_span::SPAN;
use oxc_syntax::identifier::is_identifier_name;

use crate::{
    JsCodegenError,
    js::plan::{JsFunctionPlan, JsModulePlan},
};

use super::{function::emit_function_node, region::emit_expression_region};

pub(crate) fn emit_object_literal_expression<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    output_plan: &JsModulePlan,
    function: &JsFunctionIr,
    plan: &JsFunctionPlan,
    object: &ObjectLiteralOp,
) -> Result<Expression<'ast>, JsCodegenError> {
    let mut properties = ArenaVec::with_capacity_in(object.entries().len(), builder);

    for entry in object.entries() {
        let property = match entry {
            ObjectLiteralEntry::Property { key, value } => {
                let (key, computed) =
                    emit_object_key(builder, module, output_plan, function, plan, key)?;
                let value =
                    emit_expression_region(builder, module, output_plan, function, plan, *value)?;

                ObjectPropertyKind::new_object_property(
                    SPAN,
                    PropertyKind::Init,
                    key,
                    value,
                    false,
                    false,
                    computed,
                    builder,
                )
            }

            ObjectLiteralEntry::Spread { expression } => ObjectPropertyKind::new_spread_property(
                SPAN,
                emit_expression_region(builder, module, output_plan, function, plan, *expression)?,
                builder,
            ),

            ObjectLiteralEntry::Prototype { expression } => {
                ObjectPropertyKind::new_object_property(
                    SPAN,
                    PropertyKind::Init,
                    emit_static_object_key(builder, "__proto__"),
                    emit_expression_region(
                        builder,
                        module,
                        output_plan,
                        function,
                        plan,
                        *expression,
                    )?,
                    false,
                    false,
                    false,
                    builder,
                )
            }

            ObjectLiteralEntry::Method {
                kind,
                key,
                function: method_function,
            } => {
                let method_ir =
                    module
                        .function(*method_function)
                        .ok_or(JsCodegenError::UnknownFunction {
                            function: *method_function,
                        })?;

                if method_ir.kind() != FunctionKind::ObjectMethod {
                    return Err(JsCodegenError::InvalidFunctionKind {
                        function: *method_function,
                    });
                }

                let (key, computed) =
                    emit_object_key(builder, module, output_plan, function, plan, key)?;
                let property_kind = match kind {
                    ObjectMethodKind::Method => PropertyKind::Init,
                    ObjectMethodKind::Getter => PropertyKind::Get,
                    ObjectMethodKind::Setter => PropertyKind::Set,
                };

                ObjectPropertyKind::new_object_property(
                    SPAN,
                    property_kind,
                    key,
                    Expression::FunctionExpression(emit_function_node(
                        builder,
                        module,
                        output_plan,
                        *method_function,
                        None,
                    )?),
                    true,
                    false,
                    computed,
                    builder,
                )
            }
        };

        properties.push(property);
    }

    Ok(Expression::new_object_expression(SPAN, properties, builder))
}

fn emit_object_key<'ast>(
    builder: &AstBuilder<'ast>,
    module: &JsModuleIr,
    output_plan: &JsModulePlan,
    function: &JsFunctionIr,
    plan: &JsFunctionPlan,
    key: &ObjectLiteralKey,
) -> Result<(AstPropertyKey<'ast>, bool), JsCodegenError> {
    match key {
        ObjectLiteralKey::Static(name) => Ok((emit_static_object_key(builder, name), false)),
        ObjectLiteralKey::Computed { expression } => Ok((
            AstPropertyKey::from(emit_expression_region(
                builder,
                module,
                output_plan,
                function,
                plan,
                *expression,
            )?),
            true,
        )),
    }
}

pub(crate) fn emit_static_object_key<'ast>(
    builder: &AstBuilder<'ast>,
    name: &str,
) -> AstPropertyKey<'ast> {
    let name = builder.allocator().alloc_str(name);

    if is_identifier_name(name) {
        AstPropertyKey::new_static_identifier(SPAN, name, builder)
    } else {
        AstPropertyKey::from(Expression::new_string_literal(SPAN, name, None, builder))
    }
}
