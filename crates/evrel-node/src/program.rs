//! Node-API types for whole-program compilation.

use evrel_compiler::{
    ModuleAttribute as CompilerModuleAttribute, ModuleExportName as CompilerModuleExportName,
    ModuleKey, ModuleRequest as CompilerModuleRequest,
    ModuleRequestKind as CompilerModuleRequestKind, ProgramInput as CompilerProgramInput,
    ProgramModuleInput as CompilerProgramModuleInput, ProgramOutput as CompilerProgramOutput,
    ResolvedModuleRequest as CompilerResolvedModuleRequest,
    ResolvedModuleTarget as CompilerResolvedModuleTarget,
    compile_program as compiler_compile_program,
};
use napi::{
    Env, Task,
    bindgen_prelude::{AsyncTask, Result},
};
use napi_derive::napi;

use crate::into_node_error;

/// A complete host-resolved source program.
#[napi(object)]
pub struct ProgramInput {
    pub modules: Vec<ProgramModuleInput>,
    pub entrypoints: Vec<String>,
}

/// One transformed source module.
#[napi(object)]
pub struct ProgramModuleInput {
    pub key: String,
    pub filename: String,
    pub source: String,
    pub resolved_requests: Vec<ResolvedModuleRequest>,
}

/// One host-resolved module request.
#[napi(object)]
pub struct ResolvedModuleRequest {
    pub kind: ModuleRequestKind,
    pub specifier: String,
    pub attributes: Vec<ModuleAttribute>,
    pub target: ResolvedModuleTarget,
}

/// How source requested another module.
#[napi(string_enum = "camelCase")]
pub enum ModuleRequestKind {
    StaticImport,
    ReExport,
    DynamicImport,
    CommonJsRequire,
}

/// One import attribute.
#[napi(object)]
pub struct ModuleAttribute {
    pub key: String,
    pub value: String,
}

/// A host-resolved request target.
#[napi(object)]
pub struct ResolvedModuleTarget {
    pub kind: ResolvedModuleTargetKind,
    pub key: String,
}

/// How the target participates in compilation.
#[napi(string_enum = "camelCase")]
pub enum ResolvedModuleTargetKind {
    Internal,
    Opaque,
    External,
}

/// Generated JavaScript for a complete program.
#[napi(object)]
pub struct ProgramOutput {
    pub modules: Vec<GeneratedModule>,
}

/// Generated JavaScript for one source module.
#[napi(object)]
pub struct GeneratedModule {
    pub key: String,
    pub code: String,
}

/// Compiles a complete source program without blocking Node.js.
#[napi(js_name = "compileProgram", ts_return_type = "Promise<ProgramOutput>")]
pub fn compile_program(input: ProgramInput) -> AsyncTask<CompileProgramTask> {
    AsyncTask::new(CompileProgramTask::new(input))
}

#[doc(hidden)]
pub struct CompileProgramTask {
    input: Option<ProgramInput>,
}

impl CompileProgramTask {
    fn new(input: ProgramInput) -> Self {
        Self { input: Some(input) }
    }
}

impl Task for CompileProgramTask {
    type Output = CompilerProgramOutput;
    type JsValue = ProgramOutput;

    fn compute(&mut self) -> Result<Self::Output> {
        let input = self
            .input
            .take()
            .expect("a compilation task must execute only once");

        compiler_compile_program(input.into()).map_err(into_node_error)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output.into())
    }
}

impl From<ProgramInput> for CompilerProgramInput {
    fn from(input: ProgramInput) -> Self {
        Self::new(
            input.modules.into_iter().map(Into::into),
            input.entrypoints.into_iter().map(ModuleKey::new),
        )
    }
}

impl From<ProgramModuleInput> for CompilerProgramModuleInput {
    fn from(input: ProgramModuleInput) -> Self {
        Self::new(ModuleKey::new(input.key), input.filename, input.source)
            .with_resolved_requests(input.resolved_requests.into_iter().map(Into::into))
    }
}

impl From<ResolvedModuleRequest> for CompilerResolvedModuleRequest {
    fn from(request: ResolvedModuleRequest) -> Self {
        Self::new(
            CompilerModuleRequest::new(
                request.kind.into(),
                request.specifier,
                request
                    .attributes
                    .into_iter()
                    .map(Into::into)
                    .collect::<Vec<_>>(),
            ),
            request.target.into(),
        )
    }
}

impl From<ModuleRequestKind> for CompilerModuleRequestKind {
    fn from(kind: ModuleRequestKind) -> Self {
        match kind {
            ModuleRequestKind::StaticImport => Self::StaticImport,
            ModuleRequestKind::ReExport => Self::ReExport,
            ModuleRequestKind::DynamicImport => Self::DynamicImport,
            ModuleRequestKind::CommonJsRequire => Self::CommonJsRequire,
        }
    }
}

impl From<CompilerModuleRequestKind> for ModuleRequestKind {
    fn from(kind: CompilerModuleRequestKind) -> Self {
        match kind {
            CompilerModuleRequestKind::StaticImport => Self::StaticImport,
            CompilerModuleRequestKind::ReExport => Self::ReExport,
            CompilerModuleRequestKind::DynamicImport => Self::DynamicImport,
            CompilerModuleRequestKind::CommonJsRequire => Self::CommonJsRequire,
        }
    }
}

impl From<ModuleAttribute> for CompilerModuleAttribute {
    fn from(attribute: ModuleAttribute) -> Self {
        Self::new(
            CompilerModuleExportName::String(attribute.key.into()),
            attribute.value,
        )
    }
}

impl From<ResolvedModuleTarget> for CompilerResolvedModuleTarget {
    fn from(target: ResolvedModuleTarget) -> Self {
        let key = ModuleKey::new(target.key);

        match target.kind {
            ResolvedModuleTargetKind::Internal => Self::Internal(key),
            ResolvedModuleTargetKind::Opaque => Self::Opaque(key),
            ResolvedModuleTargetKind::External => Self::External(key),
        }
    }
}

impl From<CompilerProgramOutput> for ProgramOutput {
    fn from(output: CompilerProgramOutput) -> Self {
        Self {
            modules: output
                .into_modules()
                .into_iter()
                .map(|module| {
                    let (key, code) = module.into_parts();

                    GeneratedModule {
                        key: key.as_str().into(),
                        code,
                    }
                })
                .collect(),
        }
    }
}
