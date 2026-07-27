//! Host-provided inputs for whole-program compilation.

use evrel_ir::{ModuleKey, ModuleRequest};

/// A complete source program submitted for compilation.
///
/// Resolution has already been performed by the host, such as Vite. The
/// compiler links the stable module keys to compiler-owned module IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramInput {
    modules: Box<[ProgramModuleInput]>,
    entrypoints: Box<[ModuleKey]>,
}

impl ProgramInput {
    /// Creates a source program from its modules and entrypoints.
    pub fn new(
        modules: impl IntoIterator<Item = ProgramModuleInput>,
        entrypoints: impl IntoIterator<Item = ModuleKey>,
    ) -> Self {
        Self {
            modules: modules.into_iter().collect(),
            entrypoints: entrypoints.into_iter().collect(),
        }
    }

    /// Returns source modules in host-provided order.
    pub fn modules(&self) -> &[ProgramModuleInput] {
        &self.modules
    }

    /// Returns the program's root modules.
    pub fn entrypoints(&self) -> &[ModuleKey] {
        &self.entrypoints
    }
}

/// One source module supplied by the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramModuleInput {
    key: ModuleKey,
    source_name: Box<str>,
    source_text: Box<str>,
    resolved_requests: Box<[ResolvedModuleRequest]>,
}

impl ProgramModuleInput {
    /// Creates a resolved source module.
    pub fn new(
        key: ModuleKey,
        source_name: impl Into<Box<str>>,
        source_text: impl Into<Box<str>>,
    ) -> Self {
        Self {
            key,
            source_name: source_name.into(),
            source_text: source_text.into(),
            resolved_requests: Box::new([]),
        }
    }

    /// Adds this module's host-resolved requests.
    pub fn with_resolved_requests(
        mut self,
        requests: impl IntoIterator<Item = ResolvedModuleRequest>,
    ) -> Self {
        self.resolved_requests = requests.into_iter().collect();
        self
    }

    /// Returns the module's canonical host identity.
    pub const fn key(&self) -> &ModuleKey {
        &self.key
    }

    /// Returns the parser and diagnostic source name.
    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    /// Returns the host-transformed source code.
    pub fn source_text(&self) -> &str {
        &self.source_text
    }

    /// Returns host-resolved requests in source order.
    pub fn resolved_requests(&self) -> &[ResolvedModuleRequest] {
        &self.resolved_requests
    }
}

/// A module request already resolved by the host.
///
/// It still uses stable module keys because IR module IDs have not been
/// allocated yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedModuleRequest {
    request: ModuleRequest,
    target: ResolvedModuleTarget,
}

impl ResolvedModuleRequest {
    /// Creates a host-resolved module request.
    pub fn new(request: ModuleRequest, target: ResolvedModuleTarget) -> Self {
        Self { request, target }
    }

    /// Returns the unresolved source-level request.
    pub const fn request(&self) -> &ModuleRequest {
        &self.request
    }

    /// Returns the host-resolved target.
    pub const fn target(&self) -> &ResolvedModuleTarget {
        &self.target
    }
}

/// The host-resolved destination of a module request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedModuleTarget {
    /// Source for this module is included in `ProgramInput`.
    Internal(ModuleKey),

    /// The host owns this module, but Evrel cannot inspect its source.
    Opaque(ModuleKey),

    /// The module remains external to the emitted program.
    External(ModuleKey),
}
