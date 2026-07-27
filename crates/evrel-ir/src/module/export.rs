//! Static JavaScript module exports.

use crate::{BindingId, ModuleAttribute, ModuleExportName};

/// A static export owned by a module.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModuleExport {
    /// Exposes a module-owned live binding under an exported name.
    Local {
        exported: ModuleExportName,
        binding: BindingId,
    },

    /// Forwards an export from another module without creating a local binding.
    Indirect {
        source: Box<str>,
        attributes: Box<[ModuleAttribute]>,
        imported: ModuleExportName,
        exported: ModuleExportName,
    },

    /// Exposes another module's namespace object under one exported name.
    Namespace {
        source: Box<str>,
        attributes: Box<[ModuleAttribute]>,
        exported: ModuleExportName,
    },

    /// Forwards all named exports except `default` from another module.
    Star {
        source: Box<str>,
        attributes: Box<[ModuleAttribute]>,
    },
}

impl ModuleExport {
    /// Creates a local binding export.
    pub fn local(exported: ModuleExportName, binding: BindingId) -> Self {
        Self::Local { exported, binding }
    }

    /// Creates an indirect export from another module.
    pub fn indirect(
        source: impl Into<Box<str>>,
        attributes: impl Into<Box<[ModuleAttribute]>>,
        imported: ModuleExportName,
        exported: ModuleExportName,
    ) -> Self {
        Self::Indirect {
            source: source.into(),
            attributes: attributes.into(),
            imported,
            exported,
        }
    }

    /// Creates a namespace export from another module.
    pub fn namespace(
        source: impl Into<Box<str>>,
        attributes: impl Into<Box<[ModuleAttribute]>>,
        exported: ModuleExportName,
    ) -> Self {
        Self::Namespace {
            source: source.into(),
            attributes: attributes.into(),
            exported,
        }
    }

    /// Creates a star export from another module.
    pub fn star(
        source: impl Into<Box<str>>,
        attributes: impl Into<Box<[ModuleAttribute]>>,
    ) -> Self {
        Self::Star {
            source: source.into(),
            attributes: attributes.into(),
        }
    }

    /// Returns the outward module name of a named export, if any.
    pub const fn exported_name(&self) -> Option<&ModuleExportName> {
        match self {
            Self::Local { exported, .. }
            | Self::Indirect { exported, .. }
            | Self::Namespace { exported, .. } => Some(exported),
            Self::Star { .. } => None,
        }
    }

    /// Returns the local live binding exposed by this export, if any.
    pub const fn binding(&self) -> Option<BindingId> {
        match self {
            Self::Local { binding, .. } => Some(*binding),
            Self::Indirect { .. } | Self::Namespace { .. } | Self::Star { .. } => None,
        }
    }

    /// Returns the source module specifier for an indirect export.
    pub fn source(&self) -> Option<&str> {
        match self {
            Self::Local { .. } => None,
            Self::Indirect { source, .. }
            | Self::Namespace { source, .. }
            | Self::Star { source, .. } => Some(source),
        }
    }

    /// Returns the source module name selected by an indirect export.
    pub const fn imported_name(&self) -> Option<&ModuleExportName> {
        match self {
            Self::Indirect { imported, .. } => Some(imported),
            Self::Local { .. } | Self::Namespace { .. } | Self::Star { .. } => None,
        }
    }

    /// Returns static export attributes in source order.
    pub fn attributes(&self) -> &[ModuleAttribute] {
        match self {
            Self::Local { .. } => &[],
            Self::Indirect { attributes, .. }
            | Self::Namespace { attributes, .. }
            | Self::Star { attributes, .. } => attributes,
        }
    }
}
