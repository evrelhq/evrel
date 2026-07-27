//! Shared JavaScript class-definition lowering.

use evrel_ir::{
    BindingKind, ClassElement as IrClassElement, ClassElementKey, ClassField, ClassFieldPlacement,
    ClassMethod, ClassMethodKind, ClassMethodPlacement, ClassStaticBlock, CreateClassOp,
    FunctionId, FunctionKind, FunctionMode, OperationKind, ReturnOp, ValueId,
};
use oxc_ast::ast::{
    Class, ClassElement as OxcClassElement, Expression, MethodDefinition, MethodDefinitionKind,
    PropertyDefinition, PropertyKey, StaticBlock as OxcStaticBlock,
};
use oxc_syntax::number::ToJsString;

use crate::FrontendError;

use super::{
    FunctionLowerer,
    declaration::{declare_scope_bindings, instantiate_function_scope},
    expression::lower_expression,
    lower_function_statements,
};

/// Lowers a class definition and returns its class-constructor value.
pub(super) fn lower_class_value(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    class: &Class<'_>,
) -> Result<ValueId, FrontendError> {
    if !class.decorators.is_empty() {
        return Err(FrontendError::UnsupportedExpression);
    }

    let self_binding = class.id.as_ref().map(|identifier| {
        let symbol = identifier.symbol_id();

        if lowerer.contains_binding(symbol) {
            lowerer.binding_for_symbol(symbol)
        } else {
            lowerer.declare_binding(symbol, identifier.name.as_str(), BindingKind::Class)
        }
    });

    let super_class = class
        .super_class
        .as_ref()
        .map(|expression| {
            lowerer.build_expression_region(|lowerer| lower_expression(lowerer, expression))
        })
        .transpose()?;
    let private_names = class
        .body
        .body
        .iter()
        .filter_map(OxcClassElement::property_key)
        .filter_map(PropertyKey::private_name)
        .map(|name| Box::<str>::from(name.as_str()))
        .collect::<Vec<_>>();
    let elements = lowerer.with_private_name_scope(private_names, |lowerer| {
        lower_class_elements(lowerer, class)
    })?;

    Ok(lowerer.emit_value(
        OperationKind::CreateClass(CreateClassOp::new(self_binding, super_class, elements)),
        [],
    ))
}

fn lower_class_elements(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    class: &Class<'_>,
) -> Result<Vec<IrClassElement>, FrontendError> {
    let mut elements = Vec::with_capacity(class.body.body.len());

    for element in &class.body.body {
        match element {
            OxcClassElement::MethodDefinition(method) => {
                elements.push(lower_method(lowerer, method)?);
            }

            OxcClassElement::PropertyDefinition(field) => {
                elements.push(lower_class_field(lowerer, field)?);
            }

            OxcClassElement::StaticBlock(block) => {
                elements.push(lower_class_static_block(lowerer, block)?);
            }

            _ => return Err(FrontendError::UnsupportedExpression),
        }
    }

    Ok(elements)
}

fn lower_class_static_block(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    block: &OxcStaticBlock<'_>,
) -> Result<IrClassElement, FrontendError> {
    let scope = block
        .scope_id
        .get()
        .expect("semantic analysis must assign the class static block scope");
    let (body, result) = lowerer.build_nested_function(
        FunctionKind::ClassStaticBlock,
        FunctionMode::Normal,
        |nested| {
            declare_scope_bindings(nested, scope)?;
            instantiate_function_scope(nested, scope, &block.body)?;
            lower_function_statements(nested, &block.body)
        },
    );

    result?;

    Ok(IrClassElement::StaticBlock(ClassStaticBlock::new(body)))
}

fn lower_method(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    method: &MethodDefinition<'_>,
) -> Result<IrClassElement, FrontendError> {
    if !method.decorators.is_empty() {
        return Err(FrontendError::UnsupportedExpression);
    }

    let kind = match method.kind {
        MethodDefinitionKind::Method => ClassMethodKind::Method,
        MethodDefinitionKind::Get => ClassMethodKind::Getter,
        MethodDefinitionKind::Set => ClassMethodKind::Setter,
        MethodDefinitionKind::Constructor => ClassMethodKind::Constructor,
    };

    let key = lower_class_element_key(lowerer, &method.key, method.computed)?;
    let placement = if method.r#static {
        ClassMethodPlacement::Static
    } else {
        ClassMethodPlacement::Prototype
    };
    let function = super::lower_class_element_function(
        lowerer,
        &method.value,
        method.kind == MethodDefinitionKind::Constructor,
    )?;

    Ok(IrClassElement::Method(ClassMethod::new(
        kind, placement, key, function,
    )))
}

fn lower_class_field(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    field: &PropertyDefinition<'_>,
) -> Result<IrClassElement, FrontendError> {
    if !field.decorators.is_empty() {
        return Err(FrontendError::UnsupportedExpression);
    }

    let key = lower_class_element_key(lowerer, &field.key, field.computed)?;
    let placement = if field.r#static {
        ClassFieldPlacement::Static
    } else {
        ClassFieldPlacement::Instance
    };
    let initializer = field
        .value
        .as_ref()
        .map(|expression| lower_class_field_initializer(lowerer, expression))
        .transpose()?;

    Ok(IrClassElement::Field(ClassField::new(
        placement,
        key,
        initializer,
    )))
}

fn lower_class_field_initializer(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &Expression<'_>,
) -> Result<FunctionId, FrontendError> {
    let (function, result) = lowerer.build_nested_function(
        FunctionKind::ClassFieldInitializer,
        FunctionMode::Normal,
        |initializer| {
            let value = lower_expression(initializer, expression)?;

            initializer.terminate(OperationKind::Return(ReturnOp::new()), [value]);

            Ok(())
        },
    );

    result?;

    Ok(function)
}

fn lower_class_element_key(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    key: &PropertyKey<'_>,
    computed: bool,
) -> Result<ClassElementKey, FrontendError> {
    if computed {
        let expression = key
            .as_expression()
            .ok_or(FrontendError::UnsupportedExpression)?;
        let region =
            lowerer.build_expression_region(|lowerer| lower_expression(lowerer, expression))?;

        return Ok(ClassElementKey::Computed(region));
    }

    match key {
        PropertyKey::StaticIdentifier(identifier) => {
            Ok(ClassElementKey::Static(identifier.name.as_str().into()))
        }

        PropertyKey::StringLiteral(literal) => {
            Ok(ClassElementKey::Static(literal.value.as_str().into()))
        }

        PropertyKey::NumericLiteral(literal) => {
            let name = if literal.value == 0.0 {
                "0".into()
            } else {
                literal.value.to_js_string().into()
            };

            Ok(ClassElementKey::Static(name))
        }

        PropertyKey::PrivateIdentifier(identifier) => Ok(ClassElementKey::Private(
            lowerer.private_name(identifier.name.as_str()),
        )),

        _ => Err(FrontendError::UnsupportedExpression),
    }
}
