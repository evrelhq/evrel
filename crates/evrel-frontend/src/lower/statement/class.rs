//! JavaScript class-declaration lowering.

use evrel_ir::{BindingId, InitializeBindingOp, OperationKind};
use oxc_ast::ast::Class;

use crate::{
    FrontendError,
    lower::{FunctionLowerer, class::lower_class_value},
};

/// Lowers a named class declaration and initializes its outer binding.
pub(super) fn lower_class_declaration(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    class: &Class<'_>,
) -> Result<(), FrontendError> {
    let identifier = class
        .id
        .as_ref()
        .expect("ordinary class declarations must have a name");
    let binding = lowerer.binding_for_symbol(identifier.symbol_id());

    initialize_class_binding(lowerer, class, binding)
}

/// Lowers a default-exported class and initializes its export binding.
pub(super) fn lower_default_class_declaration(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    class: &Class<'_>,
) -> Result<(), FrontendError> {
    let binding = match &class.id {
        Some(identifier) => lowerer.binding_for_symbol(identifier.symbol_id()),

        None => lowerer
            .default_export_binding()
            .expect("anonymous default class must have a synthetic binding"),
    };

    initialize_class_binding(lowerer, class, binding)
}

fn initialize_class_binding(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    class: &Class<'_>,
    binding: BindingId,
) -> Result<(), FrontendError> {
    let value = lower_class_value(lowerer, class)?;

    lowerer.emit(
        OperationKind::InitializeBinding(InitializeBindingOp::new(binding)),
        [value],
    );

    Ok(())
}
