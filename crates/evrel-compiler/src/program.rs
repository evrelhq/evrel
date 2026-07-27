//! Construction of whole-program IR from host-provided inputs.

use std::collections::BTreeSet;

use evrel_frontend::lower_source_file;
use evrel_ir::{ModuleDependency, ModuleTarget, ProgramIr};

use crate::{CompilerError, ProgramInput, ResolvedModuleTarget};

/// Builds compiler-owned IR from a complete host-resolved program.
pub(crate) fn build_program_ir(input: &ProgramInput) -> Result<ProgramIr, CompilerError> {
    validate_program_input(input)?;

    let mut program = lower_modules(input)?;

    link_dependencies(input, &mut program);
    add_entrypoints(input, &mut program);

    Ok(program)
}

fn validate_program_input(input: &ProgramInput) -> Result<(), CompilerError> {
    let mut module_keys = BTreeSet::new();

    for module in input.modules() {
        if !module_keys.insert(module.key().clone()) {
            return Err(CompilerError::DuplicateProgramModule {
                module: module.key().as_str().into(),
            });
        }
    }

    for entrypoint in input.entrypoints() {
        if !module_keys.contains(entrypoint) {
            return Err(CompilerError::UnknownProgramEntrypoint {
                module: entrypoint.as_str().into(),
            });
        }
    }

    for module in input.modules() {
        for request in module.resolved_requests() {
            let ResolvedModuleTarget::Internal(target) = request.target() else {
                continue;
            };

            if !module_keys.contains(target) {
                return Err(CompilerError::UnknownInternalModule {
                    importer: module.key().as_str().into(),
                    specifier: request.request().specifier().into(),
                    target: target.as_str().into(),
                });
            }
        }
    }

    Ok(())
}

fn lower_modules(input: &ProgramInput) -> Result<ProgramIr, CompilerError> {
    let mut program = ProgramIr::new();

    for module in input.modules() {
        let ir =
            lower_source_file(module.source_name(), module.source_text()).map_err(|source| {
                CompilerError::ProgramModule {
                    module: module.key().as_str().into(),
                    source: Box::new(CompilerError::Frontend(source)),
                }
            })?;

        program.add_module(module.key().clone(), ir);
    }

    Ok(program)
}

fn link_dependencies(input: &ProgramInput, program: &mut ProgramIr) {
    for module in input.modules() {
        let importer = program
            .module_by_key(module.key())
            .expect("every validated module was allocated");

        for request in module.resolved_requests() {
            let target = match request.target() {
                ResolvedModuleTarget::Internal(key) => ModuleTarget::Internal(
                    program
                        .module_by_key(key)
                        .expect("every validated internal target was allocated"),
                ),
                ResolvedModuleTarget::Opaque(key) => ModuleTarget::Opaque(key.clone()),
                ResolvedModuleTarget::External(key) => ModuleTarget::External(key.clone()),
            };

            program.add_dependency(ModuleDependency::new(
                importer,
                request.request().clone(),
                target,
            ));
        }
    }
}

fn add_entrypoints(input: &ProgramInput, program: &mut ProgramIr) {
    for key in input.entrypoints() {
        let module = program
            .module_by_key(key)
            .expect("every validated entrypoint was allocated");

        program.add_entry_module(module);
    }
}
