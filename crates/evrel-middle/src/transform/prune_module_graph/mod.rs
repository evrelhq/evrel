//! Pruning of unreachable modules and module-interface entries.

use evrel_ir::ProgramIr;

use crate::analysis::ProgramReachability;

mod interface;
mod modules;

/// Counts of module-graph entities removed while applying reachability.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModuleGraphPruning {
    removed_modules: usize,
    removed_interface_entries: usize,
}

impl ModuleGraphPruning {
    /// Returns the number of removed modules.
    pub const fn removed_modules(self) -> usize {
        self.removed_modules
    }

    /// Returns the number of removed import bindings and export entries.
    pub const fn removed_interface_entries(self) -> usize {
        self.removed_interface_entries
    }
}

/// Removes unreachable modules and module-interface entries.
///
/// Unreachable modules are removed before the import/export interface of each
/// retained module is pruned.
pub fn prune_module_graph(
    program: &mut ProgramIr,
    reachability: &ProgramReachability,
) -> ModuleGraphPruning {
    let removed_modules = modules::prune(program, reachability);
    let modules = program
        .modules()
        .map(|(module, _)| module)
        .collect::<Vec<_>>();
    let mut removed_interface_entries = 0;

    for module in modules {
        let ir = program
            .module_ir_mut(module)
            .expect("collected program module must remain live");

        removed_interface_entries += interface::prune(module, ir, reachability);
    }

    ModuleGraphPruning {
        removed_modules,
        removed_interface_entries,
    }
}
