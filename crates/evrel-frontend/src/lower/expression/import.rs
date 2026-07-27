//! JavaScript dynamic-import lowering.

use evrel_ir::{DynamicImportOp, DynamicImportPhase, OperationKind, ValueId};
use oxc_ast::ast::{ImportExpression, ImportPhase};

use crate::{FrontendError, lower::FunctionLowerer};

use super::lower_expression;

/// Lowers a dynamic import expression in source evaluation order.
pub(super) fn lower_import_expression(
    lowerer: &mut FunctionLowerer<'_, '_, '_>,
    expression: &ImportExpression<'_>,
) -> Result<ValueId, FrontendError> {
    let source = lower_expression(lowerer, &expression.source)?;
    let mut operands = vec![source];

    if let Some(options) = &expression.options {
        operands.push(lower_expression(lowerer, options)?);
    }

    let phase = match expression.phase {
        None => DynamicImportPhase::Evaluation,
        Some(ImportPhase::Source) => DynamicImportPhase::Source,
        Some(ImportPhase::Defer) => DynamicImportPhase::Defer,
    };

    Ok(lowerer.emit_value(
        OperationKind::DynamicImport(DynamicImportOp::new(phase, expression.options.is_some())),
        operands,
    ))
}
