//! Default-export expression lowering.

use evrel_ir::{InitializeBindingOp, OperationKind};
use oxc_ast::ast::{ExportDefaultDeclaration, ExportDefaultDeclarationKind};

use crate::{
    FrontendError,
    lower::{FunctionLowerer, expression::lower_expression},
};

use super::class::lower_default_class_declaration;

pub(super) fn lower_export_default_declaration(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    declaration: &ExportDefaultDeclaration<'_>,
) -> Result<(), FrontendError> {
    match &declaration.declaration {
        ExportDefaultDeclarationKind::FunctionDeclaration(_) => {
            // Function declarations are emitted during declaration instantiation.
            return Ok(());
        }

        ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            return lower_default_class_declaration(lowerer, class);
        }

        _ => {}
    }

    let expression = declaration
        .declaration
        .as_expression()
        .ok_or(FrontendError::UnsupportedStatement)?;
    let value = lower_expression(lowerer, expression)?;
    let binding = lowerer
        .default_export_binding()
        .expect("default-export expression must have a synthetic binding");

    lowerer.emit(
        OperationKind::InitializeBinding(InitializeBindingOp::new(binding)),
        [value],
    );

    Ok(())
}
