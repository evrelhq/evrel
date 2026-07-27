//! JavaScript variable-declaration lowering.

use evrel_ir::{BindingId, BindingWriteMode, ConstantOp, ConstantValue, OperationKind};
use oxc_ast::ast::{
    BindingPattern as OxcBindingPattern, VariableDeclaration, VariableDeclarationKind,
    VariableDeclarator,
};

use crate::{
    FrontendError,
    lower::{
        FunctionLowerer,
        expression::lower_expression,
        pattern::{emit_binding_pattern_write, lower_binding_pattern},
    },
};

/// Lowers a declaration and returns its bindings in `BoundNames` order.
pub(super) fn lower_variable_declaration(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    declaration: &VariableDeclaration<'_>,
) -> Result<Box<[BindingId]>, FrontendError> {
    if !matches!(
        declaration.kind,
        VariableDeclarationKind::Const
            | VariableDeclarationKind::Let
            | VariableDeclarationKind::Var
    ) {
        return Err(FrontendError::UnsupportedVariableDeclarationKind {
            kind: declaration.kind.as_str().into(),
        });
    }

    let mut bindings = Vec::new();

    for declarator in &declaration.declarations {
        bindings.extend(lower_variable_declarator(lowerer, declarator)?);
    }

    Ok(bindings.into_boxed_slice())
}

fn lower_variable_declarator(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    declarator: &VariableDeclarator<'_>,
) -> Result<Vec<BindingId>, FrontendError> {
    let pattern = lower_binding_pattern(lowerer, &declarator.id)?;
    let bindings = pattern.binding_ids();

    let value = match &declarator.init {
        Some(initializer) => lower_expression(lowerer, initializer)?,

        None if pattern.as_binding().is_none() => {
            return Err(FrontendError::MissingDestructuringInitializer);
        }

        None if declarator.kind == VariableDeclarationKind::Var => {
            return Ok(bindings);
        }

        None if declarator.kind == VariableDeclarationKind::Let => lowerer.emit_value(
            OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
            [],
        ),

        None => {
            let OxcBindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                unreachable!("destructuring without an initializer was rejected");
            };

            return Err(FrontendError::MissingBindingInitializer {
                name: identifier.name.as_str().into(),
            });
        }
    };

    let mode = match declarator.kind {
        VariableDeclarationKind::Var => BindingWriteMode::Store,

        VariableDeclarationKind::Let | VariableDeclarationKind::Const => {
            BindingWriteMode::Initialize
        }

        _ => unreachable!("unsupported declaration kinds were rejected"),
    };

    emit_binding_pattern_write(lowerer, pattern, mode, value);

    Ok(bindings)
}
