//! Removal of modules outside the program entry dependency closure.

use evrel_js_ir::{JsProgramIr, ProgramEditor};

use crate::js::analysis::ProgramReachability;

/// Removes modules unreachable from every program entry.
///
/// All dependency kinds are followed conservatively. Missing static linkage
/// causes reachability to retain the entire program.
///
/// Returns the number of removed modules.
pub(super) fn prune(program: &mut JsProgramIr, reachability: &ProgramReachability) -> usize {
    let unreachable = program
        .modules()
        .map(|(module, _)| module)
        .filter(|module| !reachability.is_module_evaluated(*module))
        .collect::<Vec<_>>();
    let removed = unreachable.len();

    if !unreachable.is_empty() {
        ProgramEditor::new(program).remove_modules(unreachable);
    }

    removed
}

#[cfg(test)]
mod tests {
    use evrel_js_ir::{
        JsModuleIr, JsProgramIr, ModuleDependency, ModuleKey, ModuleRequest, ModuleRequestKind,
        ModuleTarget,
    };

    use crate::js::analysis::{ProgramLinkage, ProgramReachability};

    use super::prune;

    #[test]
    fn removes_modules_outside_the_entry_dependency_closure() {
        let mut program = JsProgramIr::new();
        let entry = program.add_module(ModuleKey::new("entry"), JsModuleIr::new());
        let dependency = program.add_module(ModuleKey::new("dependency"), JsModuleIr::new());
        let disconnected_key = ModuleKey::new("disconnected");
        let disconnected = program.add_module(disconnected_key.clone(), JsModuleIr::new());

        program.add_entry_module(entry);
        program.add_dependency(ModuleDependency::new(
            entry,
            ModuleRequest::new(ModuleRequestKind::CommonJsRequire, "dependency", []),
            ModuleTarget::Internal(dependency),
        ));

        let linkage = ProgramLinkage::analyze(&program);
        let reachability = ProgramReachability::compute(&program, &linkage);
        assert_eq!(prune(&mut program, &reachability), 1);
        assert!(program.module(entry).is_some());
        assert!(program.module(dependency).is_some());
        assert!(program.module(disconnected).is_none());
        assert_eq!(program.module_by_key(&disconnected_key), None);
        assert_eq!(prune(&mut program, &reachability), 0);
    }
}
