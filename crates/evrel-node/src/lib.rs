//! Node-API bindings for the Evrel compiler.

#[cfg_attr(
    test,
    allow(dead_code, reason = "N-API exports are consumed by JavaScript")
)]
mod program;

use evrel_compiler::{
    CompileInput as CompilerInput, CompileOutput as CompilerOutput, CompilerError,
    compile as compiler_compile,
};
use napi::{
    Env, Task,
    bindgen_prelude::{AsyncTask, Error, Result},
};
use napi_derive::napi;

/// Options controlling one compilation.
#[napi(object)]
pub struct CompileOptions {
    /// Filename used to determine JavaScript, TypeScript, JSX, and module semantics.
    pub filename: String,
}

/// Output produced by the Evrel compiler.
#[napi(object)]
pub struct CompileOutput {
    /// Generated JavaScript.
    pub code: String,
}

/// Compiles a source file without blocking the Node.js event loop.
#[napi(js_name = "compile", ts_return_type = "Promise<CompileOutput>")]
pub fn compile(source: String, options: CompileOptions) -> AsyncTask<CompileTask> {
    AsyncTask::new(CompileTask::new(options.filename, source))
}

/// Compiles a source file synchronously.
#[napi(js_name = "compileSync")]
pub fn compile_sync(source: String, options: CompileOptions) -> Result<CompileOutput> {
    compile_source(&options.filename, &source).map(Into::into)
}

#[doc(hidden)]
pub struct CompileTask {
    source_name: String,
    source_text: String,
}

impl CompileTask {
    fn new(source_name: String, source_text: String) -> Self {
        Self {
            source_name,
            source_text,
        }
    }
}

impl Task for CompileTask {
    type Output = CompilerOutput;
    type JsValue = CompileOutput;

    fn compute(&mut self) -> Result<Self::Output> {
        compile_source(&self.source_name, &self.source_text)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(CompileOutput::from(output))
    }
}

impl From<CompilerOutput> for CompileOutput {
    fn from(output: CompilerOutput) -> Self {
        Self {
            code: output.into_code(),
        }
    }
}

fn compile_source(source_name: &str, source_text: &str) -> Result<CompilerOutput> {
    compiler_compile(CompilerInput::new(source_name, source_text)).map_err(into_node_error)
}

fn into_node_error(error: CompilerError) -> Error {
    Error::from_reason(error.to_string())
}
