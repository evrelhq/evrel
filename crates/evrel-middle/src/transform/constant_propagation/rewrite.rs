//! Planning and application of constant replacements.

use evrel_ir::{ConstantValue, FunctionEditor, FunctionIr, OperationId, OperationKind};

use crate::analysis::FunctionValueAnalysis;

/// Collects replacements without mutating the analyzed function snapshot.
pub(super) fn plan_constant_replacements(
    function: &FunctionIr,
    analysis: &FunctionValueAnalysis,
) -> Vec<(OperationId, ConstantValue)> {
    function
        .operations()
        .filter_map(|(operation, data)| {
            if matches!(data.kind(), OperationKind::Constant(_))
                || data.results().len() != 1
                || !data.regions().is_empty()
            {
                return None;
            }

            if !function
                .operation_effects(operation)
                .expect("live operation must have an effect summary")
                .is_empty()
            {
                return None;
            }

            let constant = analysis.value(data.results()[0]).constant()?.clone();

            Some((operation, constant))
        })
        .collect()
}

/// Applies replacements planned against an immutable function snapshot.
pub(super) fn rewrite_constants(
    function: &mut FunctionIr,
    replacements: Vec<(OperationId, ConstantValue)>,
) -> usize {
    let replaced = replacements.len();
    let mut editor = FunctionEditor::new(function);

    for (operation, constant) in replacements {
        editor.replace_operation_with_constant(operation, constant);
    }

    replaced
}
