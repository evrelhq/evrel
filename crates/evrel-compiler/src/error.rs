//! Errors produced by compiler orchestration.

use evrel_frontend::FrontendError;
use thiserror::Error;

/// Failure to compile a source module.
#[derive(Debug, Error)]
pub enum CompilerError {
    /// The program contains the same canonical module more than once.
    #[error("program contains duplicate module `{module}`")]
    DuplicateProgramModule { module: Box<str> },

    /// A program entrypoint does not refer to an included module.
    #[error("program entrypoint `{module}` is not included in the program")]
    UnknownProgramEntrypoint { module: Box<str> },

    /// An internal request targets a module missing from the program.
    #[error("module `{importer}` resolves `{specifier}` to missing internal module `{target}`")]
    UnknownInternalModule {
        importer: Box<str>,
        specifier: Box<str>,
        target: Box<str>,
    },

    /// One module in a program failed to compile.
    #[error("failed to compile program module `{module}`: {source}")]
    ProgramModule {
        module: Box<str>,
        #[source]
        source: Box<Self>,
    },

    /// Parsing or frontend lowering failed.
    #[error(transparent)]
    Frontend(#[from] FrontendError),

    /// JavaScript output planning or emission failed.
    #[error(transparent)]
    JavaScriptCodegen(#[from] evrel_codegen_js::JsCodegenError),
}
