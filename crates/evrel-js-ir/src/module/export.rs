//! Static JavaScript module exports.

use crate::{BindingId, LocationId, ModuleAttribute, ModuleExportName};

/// A static export owned by a module.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModuleExport {
    /// Evaluates a re-exported module without exposing one of its names.
    Empty {
        location: LocationId,
        source: Box<str>,
        attributes: Box<[ModuleAttribute]>,
    },

    /// Exposes a module-owned live binding under an exported name.
    Local {
        location: LocationId,
        exported: ModuleExportName,
        binding: BindingId,
    },

    /// Forwards an export from another module without creating a local binding.
    Indirect {
        location: LocationId,
        source: Box<str>,
        attributes: Box<[ModuleAttribute]>,
        imported: ModuleExportName,
        exported: ModuleExportName,
    },

    /// Exposes another module's namespace object under one exported name.
    Namespace {
        location: LocationId,
        source: Box<str>,
        attributes: Box<[ModuleAttribute]>,
        exported: ModuleExportName,
    },

    /// Forwards all named exports except `default` from another module.
    Star {
        location: LocationId,
        source: Box<str>,
        attributes: Box<[ModuleAttribute]>,
    },
}

impl ModuleExport {
    /// Creates an empty re-export.
    pub fn empty(
        location: LocationId,
        source: impl Into<Box<str>>,
        attributes: impl Into<Box<[ModuleAttribute]>>,
    ) -> Self {
        Self::Empty {
            location,
            source: source.into(),
            attributes: attributes.into(),
        }
    }

    /// Creates a local binding export.
    pub fn local(location: LocationId, exported: ModuleExportName, binding: BindingId) -> Self {
        Self::Local {
            location,
            exported,
            binding,
        }
    }

    /// Creates an indirect export from another module.
    pub fn indirect(
        location: LocationId,
        source: impl Into<Box<str>>,
        attributes: impl Into<Box<[ModuleAttribute]>>,
        imported: ModuleExportName,
        exported: ModuleExportName,
    ) -> Self {
        Self::Indirect {
            location,
            source: source.into(),
            attributes: attributes.into(),
            imported,
            exported,
        }
    }

    /// Creates a namespace export from another module.
    pub fn namespace(
        location: LocationId,
        source: impl Into<Box<str>>,
        attributes: impl Into<Box<[ModuleAttribute]>>,
        exported: ModuleExportName,
    ) -> Self {
        Self::Namespace {
            location,
            source: source.into(),
            attributes: attributes.into(),
            exported,
        }
    }

    /// Creates a star export from another module.
    pub fn star(
        location: LocationId,
        source: impl Into<Box<str>>,
        attributes: impl Into<Box<[ModuleAttribute]>>,
    ) -> Self {
        Self::Star {
            location,
            source: source.into(),
            attributes: attributes.into(),
        }
    }

    /// Returns the source location that introduced this export.
    pub const fn location(&self) -> LocationId {
        match self {
            Self::Empty { location, .. }
            | Self::Local { location, .. }
            | Self::Indirect { location, .. }
            | Self::Namespace { location, .. }
            | Self::Star { location, .. } => *location,
        }
    }

    /// Returns the outward module name of a named export, if any.
    pub const fn exported_name(&self) -> Option<&ModuleExportName> {
        match self {
            Self::Local { exported, .. }
            | Self::Indirect { exported, .. }
            | Self::Namespace { exported, .. } => Some(exported),
            Self::Empty { .. } | Self::Star { .. } => None,
        }
    }

    /// Returns the local live binding exposed by this export, if any.
    pub const fn binding(&self) -> Option<BindingId> {
        match self {
            Self::Local { binding, .. } => Some(*binding),
            Self::Empty { .. }
            | Self::Indirect { .. }
            | Self::Namespace { .. }
            | Self::Star { .. } => None,
        }
    }

    /// Returns the source module specifier for an indirect export.
    pub fn source(&self) -> Option<&str> {
        match self {
            Self::Local { .. } => None,
            Self::Empty { source, .. }
            | Self::Indirect { source, .. }
            | Self::Namespace { source, .. }
            | Self::Star { source, .. } => Some(source),
        }
    }

    /// Returns the source module name selected by an indirect export.
    pub const fn imported_name(&self) -> Option<&ModuleExportName> {
        match self {
            Self::Indirect { imported, .. } => Some(imported),
            Self::Empty { .. }
            | Self::Local { .. }
            | Self::Namespace { .. }
            | Self::Star { .. } => None,
        }
    }

    /// Returns static export attributes in source order.
    pub fn attributes(&self) -> &[ModuleAttribute] {
        match self {
            Self::Local { .. } => &[],
            Self::Empty { attributes, .. }
            | Self::Indirect { attributes, .. }
            | Self::Namespace { attributes, .. }
            | Self::Star { attributes, .. } => attributes,
        }
    }
}
