//! Single-source compiler entrypoints.

use evrel_codegen_js::generate;
use evrel_frontend::lower_source_file;
use evrel_ir::ModuleIr;
use evrel_middle::transform::{promote_bindings_to_ssa, propagate_constants};

use crate::{
    CompileInput, CompileOutput, CompilerError, GeneratedModule, ProgramInput, ProgramOutput,
    program::build_program_ir,
};

/// Compiles one source file to JavaScript.
pub fn compile(input: CompileInput<'_>) -> Result<CompileOutput, CompilerError> {
    let mut module = lower_source_file(input.source_name(), input.source_text())?;

    compile_module(&mut module)
}

/// Compiles a complete host-resolved JavaScript program.
pub fn compile_program(input: ProgramInput) -> Result<ProgramOutput, CompilerError> {
    let mut program = build_program_ir(&input)?;
    let modules = program
        .modules()
        .map(|(module, data)| (module, data.key().clone()))
        .collect::<Vec<_>>();
    let mut output = Vec::with_capacity(modules.len());

    for (module, key) in modules {
        let code = compile_module(
            program
                .module_ir_mut(module)
                .expect("a collected program module must remain live"),
        )
        .map_err(|source| CompilerError::ProgramModule {
            module: key.as_str().into(),
            source: Box::new(source),
        })?
        .into_code();

        output.push(GeneratedModule::new(key, code));
    }

    Ok(ProgramOutput::new(output))
}

fn compile_module(module: &mut ModuleIr) -> Result<CompileOutput, CompilerError> {
    promote_bindings_to_ssa(module);
    propagate_constants(module);

    let code = generate(module)?;

    Ok(CompileOutput::new(code))
}
