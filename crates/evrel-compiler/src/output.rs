//! Results returned by compiler entrypoints.

use evrel_ir::ModuleKey;

/// Successfully compiled JavaScript output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOutput {
    code: String,
}

impl CompileOutput {
    pub(crate) fn new(code: String) -> Self {
        Self { code }
    }

    /// Returns the generated JavaScript.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Consumes the output and returns the generated JavaScript.
    pub fn into_code(self) -> String {
        self.code
    }
}

/// Generated JavaScript for one module in a compiled program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedModule {
    key: ModuleKey,
    code: String,
}

impl GeneratedModule {
    pub(crate) fn new(key: ModuleKey, code: String) -> Self {
        Self { key, code }
    }

    /// Returns the source module's canonical identity.
    pub const fn key(&self) -> &ModuleKey {
        &self.key
    }

    /// Returns the generated JavaScript.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Consumes the module output into its canonical key and JavaScript.
    pub fn into_parts(self) -> (ModuleKey, String) {
        (self.key, self.code)
    }
}

/// Generated JavaScript modules for one complete program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramOutput {
    modules: Vec<GeneratedModule>,
}

impl ProgramOutput {
    pub(crate) fn new(modules: Vec<GeneratedModule>) -> Self {
        Self { modules }
    }

    /// Returns generated modules in program allocation order.
    pub fn modules(&self) -> &[GeneratedModule] {
        &self.modules
    }

    /// Returns output for one canonical module key.
    pub fn module(&self, key: &ModuleKey) -> Option<&GeneratedModule> {
        self.modules.iter().find(|module| module.key() == key)
    }

    /// Consumes the output into generated modules in program allocation order.
    pub fn into_modules(self) -> Vec<GeneratedModule> {
        self.modules
    }
}
