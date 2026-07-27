//! Single-source compiler entrypoints.

use evrel_codegen_js::generate;
use evrel_frontend::lower_source_file;
use evrel_ir::ModuleIr;

use crate::{
    CompileInput, CompileOutput, CompilerError, GeneratedModule, ProgramInput, ProgramOutput,
    program::build_program_ir,
};

/// Compiles one source file to JavaScript.
pub fn compile(input: CompileInput<'_>) -> Result<CompileOutput, CompilerError> {
    let module = lower_source_file(input.source_name(), input.source_text())?;

    compile_module(&module)
}

/// Compiles a complete host-resolved JavaScript program.
pub fn compile_program(input: ProgramInput) -> Result<ProgramOutput, CompilerError> {
    let program = build_program_ir(&input)?;
    let modules = program
        .modules()
        .map(|(module, data)| (module, data.key().clone()))
        .collect::<Vec<_>>();
    let mut output = Vec::with_capacity(modules.len());

    for (module, key) in modules {
        let code = compile_module(
            program
                .module(module)
                .expect("a collected program module must remain live")
                .ir(),
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

fn compile_module(module: &ModuleIr) -> Result<CompileOutput, CompilerError> {
    let code = generate(module)?;

    Ok(CompileOutput::new(code))
}
