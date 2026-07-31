//! Removal of modules outside every program entry dependency closure.

use evrel_ir::{ProgramEditor, ProgramIr};

use crate::analysis::ProgramReachability;

/// Removes modules unreachable from every program entry.
///
/// All dependency kinds are followed conservatively. Missing static linkage
/// causes reachability to retain the entire program.
///
/// Returns the number of removed modules.
pub(super) fn prune(program: &mut ProgramIr, reachability: &ProgramReachability) -> usize {
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
    use evrel_ir::{
        ModuleDependency, ModuleIr, ModuleKey, ModuleRequest, ModuleRequestKind, ModuleTarget,
        ProgramIr,
    };

    use crate::analysis::{ProgramLinkage, ProgramReachability};

    use super::prune;

    #[test]
    fn removes_modules_outside_the_entry_dependency_closure() {
        let mut program = ProgramIr::new();
        let entry = program.add_module(ModuleKey::new("entry"), ModuleIr::new());
        let dependency = program.add_module(ModuleKey::new("dependency"), ModuleIr::new());
        let disconnected_key = ModuleKey::new("disconnected");
        let disconnected = program.add_module(disconnected_key.clone(), ModuleIr::new());

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
