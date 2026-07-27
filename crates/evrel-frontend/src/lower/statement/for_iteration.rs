//! Shared `for...in` and `for...of` left-side lowering.

use evrel_ir::{BindingId, BindingWriteMode, ValueId};
use oxc_ast::ast::{ForStatementLeft, VariableDeclarationKind};

use crate::{
    FrontendError,
    lower::{
        FunctionLowerer,
        expression::lower_assignment_target_write,
        pattern::{emit_binding_pattern_write, lower_binding_pattern},
    },
};

/// Writes one value through an iteration statement's left side and returns
/// bindings that require a fresh environment for each iteration.
pub(super) fn lower_for_iteration_left(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    left: &ForStatementLeft<'_>,
    value: ValueId,
) -> Result<Box<[BindingId]>, FrontendError> {
    if let ForStatementLeft::VariableDeclaration(declaration) = left {
        let [declarator] = declaration.declarations.as_slice() else {
            unreachable!("Oxc validates that iteration declarations contain one declarator");
        };
        debug_assert!(declarator.init.is_none());

        let pattern = lower_binding_pattern(lowerer, &declarator.id)?;
        let bindings = pattern.binding_ids().into_boxed_slice();
        let (mode, per_iteration_bindings): (BindingWriteMode, Box<[BindingId]>) =
            match declaration.kind {
                VariableDeclarationKind::Var => (BindingWriteMode::Store, Box::default()),

                VariableDeclarationKind::Let | VariableDeclarationKind::Const => {
                    (BindingWriteMode::Initialize, bindings.clone())
                }

                _ => {
                    return Err(FrontendError::UnsupportedVariableDeclarationKind {
                        kind: declaration.kind.as_str().into(),
                    });
                }
            };

        emit_binding_pattern_write(lowerer, pattern, mode, value);

        return Ok(per_iteration_bindings);
    }

    let target = left
        .as_assignment_target()
        .expect("iteration left side must be an assignment target");

    lower_assignment_target_write(lowerer, target, value)?;

    Ok(Box::default())
}
