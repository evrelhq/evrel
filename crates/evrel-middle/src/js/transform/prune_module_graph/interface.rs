//! Pruning of unreachable static module imports and exports.

use evrel_js_ir::{
    JsModuleIr, ModuleEditor, ModuleExport, ModuleId, ModuleImport, ProgramBindingId,
};

use crate::js::analysis::ProgramReachability;

/// Removes unreachable import bindings and exports without changing module
/// evaluation.
///
/// Unused binding imports become bare imports, and unused re-exports become
/// empty re-exports. Both forms preserve the original module request and its
/// observable evaluation while removing the unused symbol interface.
///
/// Returns the total number of removed import bindings and export entries.
pub(super) fn prune(
    module: ModuleId,
    ir: &mut JsModuleIr,
    reachability: &ProgramReachability,
) -> usize {
    let mut removed_bindings = Vec::new();
    let imports = ir
        .imports()
        .iter()
        .map(|import| {
            let Some(binding) = import.binding() else {
                return import.clone();
            };
            if reachability.is_binding_live(ProgramBindingId::new(module, binding)) {
                return import.clone();
            }

            removed_bindings.push(binding);
            ModuleImport::bare(
                import.location(),
                import.source(),
                import.attributes().to_vec(),
            )
        })
        .collect::<Vec<_>>();
    let exports = ir
        .exports()
        .iter()
        .enumerate()
        .filter_map(|(index, export)| {
            if reachability.is_export_live(module, index) {
                return Some(export.clone());
            }

            export.source().map(|source| {
                ModuleExport::empty(export.location(), source, export.attributes().to_vec())
            })
        })
        .collect::<Vec<_>>();
    let removed_exports = ir
        .exports()
        .iter()
        .enumerate()
        .filter(|(index, export)| {
            !matches!(export, ModuleExport::Empty { .. })
                && !reachability.is_export_live(module, *index)
        })
        .count();
    let removed = removed_bindings.len() + removed_exports;

    ModuleEditor::new(ir).replace_module_interface(imports, exports, removed_bindings);

    removed
}
