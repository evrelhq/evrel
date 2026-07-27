//! JavaScript identifier lowering.

use evrel_ir::{LoadArgumentsOp, LoadBindingOp, LoadGlobalOp, OperationKind, ValueId};
use oxc_ast::ast::IdentifierReference;

use crate::{FrontendError, lower::FunctionLowerer};

/// Lowers an identifier reference.
pub(super) fn lower_identifier(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    identifier: &IdentifierReference<'_>,
) -> Result<ValueId, FrontendError> {
    let operation = if let Some(binding) = lowerer.binding_for_reference(identifier) {
        OperationKind::LoadBinding(LoadBindingOp::new(binding))
    } else if identifier.name == "arguments" && lowerer.has_arguments_environment() {
        OperationKind::LoadArguments(LoadArgumentsOp::new())
    } else {
        OperationKind::LoadGlobal(LoadGlobalOp::new(identifier.name.as_str()))
    };

    Ok(lowerer.emit_value(operation, []))
}
