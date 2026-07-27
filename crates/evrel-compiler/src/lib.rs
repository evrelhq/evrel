//! High-level Evrel compiler orchestration.

mod compile;
mod error;
mod input;
mod output;
mod program;
mod program_input;

pub use compile::{compile, compile_program};
pub use error::CompilerError;
pub use evrel_ir::{
    ModuleAttribute, ModuleExportName, ModuleKey, ModuleRequest, ModuleRequestKind,
};
pub use input::CompileInput;
pub use output::{CompileOutput, GeneratedModule, ProgramOutput};
pub use program_input::{
    ProgramInput, ProgramModuleInput, ResolvedModuleRequest, ResolvedModuleTarget,
};
