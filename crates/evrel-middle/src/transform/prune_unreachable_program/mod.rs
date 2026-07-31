//! Coordinated removal of unreachable program entities.

use evrel_ir::ProgramIr;

use crate::analysis::ProgramReachability;

mod bindings;
mod interface;
mod modules;

/// Counts of entities removed while applying program reachability.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProgramPruning {
    removed_modules: usize,
    removed_interface_entries: usize,
    removed_bindings: usize,
}

impl ProgramPruning {
    /// Returns the number of removed modules.
    pub const fn removed_modules(self) -> usize {
        self.removed_modules
    }

    /// Returns the number of removed import bindings and export entries.
    pub const fn removed_interface_entries(self) -> usize {
        self.removed_interface_entries
    }

    /// Returns the number of removed module bindings.
    pub const fn removed_bindings(self) -> usize {
        self.removed_bindings
    }
}

/// Applies a program-reachability result in dependency order.
///
/// Unreachable modules are removed first. Each retained module then has its
/// import/export interface pruned before its now-unreferenced bindings.
pub fn prune_unreachable_program(
    program: &mut ProgramIr,
    reachability: &ProgramReachability,
) -> ProgramPruning {
    let removed_modules = modules::prune(program, reachability);
    let modules = program
        .modules()
        .map(|(module, _)| module)
        .collect::<Vec<_>>();
    let mut removed_interface_entries = 0;
    let mut removed_bindings = 0;

    for module in modules {
        let ir = program
            .module_ir_mut(module)
            .expect("collected program module must remain live");

        removed_interface_entries += interface::prune(module, ir, reachability);
        removed_bindings += bindings::prune(module, ir, reachability);
    }

    ProgramPruning {
        removed_modules,
        removed_interface_entries,
        removed_bindings,
    }
}
