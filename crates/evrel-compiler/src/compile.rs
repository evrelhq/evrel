//! Single-source compiler entrypoints.

use evrel_codegen_js::generate;
use evrel_frontend::lower_source_file;
use evrel_ir::{FunctionIr, ModuleIr};
use evrel_middle::transform::{
    eliminate_common_subexpressions, eliminate_dead_code, promote_bindings_to_ssa,
    propagate_constants, prune_unreachable_blocks, simplify_block_parameters,
    simplify_control_flow, simplify_operations,
};
use rayon::prelude::*;

use crate::{
    CompileInput, CompileOutput, CompilerError, GeneratedModule, ProgramInput, ProgramOutput,
    program::build_program_ir,
};

const PARALLEL_FUNCTION_OPTIMIZATION_MIN_OPERATIONS: usize = 4_096;

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

    let operation_count = module
        .functions()
        .map(|(_, function)| function.operation_count())
        .sum();

    if should_parallelize_functions(module.function_count(), operation_count) {
        let mut functions = module
            .functions_mut()
            .map(|(_, function)| function)
            .collect::<Vec<_>>();

        functions
            .par_iter_mut()
            .for_each(|function| optimize_function(function));
    } else {
        module
            .functions_mut()
            .for_each(|(_, function)| optimize_function(function));
    }

    let code = generate(module)?;

    Ok(CompileOutput::new(code))
}

fn optimize_function(function: &mut FunctionIr) {
    propagate_constants(function);
    simplify_operations(function);

    simplify_control_flow(function);
    prune_unreachable_blocks(function);

    simplify_block_parameters(function);
    simplify_control_flow(function);
    eliminate_common_subexpressions(function);
    eliminate_dead_code(function);
}

fn should_parallelize_functions(function_count: usize, operation_count: usize) -> bool {
    function_count > 1
        && operation_count >= PARALLEL_FUNCTION_OPTIMIZATION_MIN_OPERATIONS
        && rayon::current_num_threads() > 1
}
