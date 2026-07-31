//! Planning and application of constant replacements.

use evrel_js_ir::{ConstantValue, FunctionEditor, JsFunctionIr, OperationId, OperationKind};

use crate::analysis::{FunctionValueAnalysis, is_safe_to_remove};

/// Collects replacements without mutating the analyzed function snapshot.
pub(super) fn plan_constant_replacements(
    function: &JsFunctionIr,
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

            if !is_safe_to_remove(function, analysis, operation) {
                return None;
            }

            let constant = analysis.value(data.results()[0]).constant()?.clone();

            Some((operation, constant))
        })
        .collect()
}

/// Applies replacements planned against an immutable function snapshot.
pub(super) fn rewrite_constants(
    function: &mut JsFunctionIr,
    replacements: Vec<(OperationId, ConstantValue)>,
) -> usize {
    let replaced = replacements.len();
    let mut editor = FunctionEditor::new(function);

    for (operation, constant) in replacements {
        editor.replace_operation_with_constant(operation, constant);
    }

    replaced
}
