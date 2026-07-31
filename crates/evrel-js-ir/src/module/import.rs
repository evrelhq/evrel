//! Static JavaScript module imports.

use crate::{BindingId, LocationId, ModuleAttribute, ModuleExportName};

/// A static import owned by a module.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModuleImport {
    /// Loads and evaluates a module without creating a local binding.
    Bare {
        location: LocationId,
        source: Box<str>,
        attributes: Box<[ModuleAttribute]>,
    },

    /// Imports the source module's default export as a local live binding.
    Default {
        location: LocationId,
        source: Box<str>,
        attributes: Box<[ModuleAttribute]>,
        binding: BindingId,
    },

    /// Imports the source module's namespace object as a local live binding.
    Namespace {
        location: LocationId,
        source: Box<str>,
        attributes: Box<[ModuleAttribute]>,
        binding: BindingId,
    },

    /// Imports one named export as a local live binding.
    Named {
        location: LocationId,
        source: Box<str>,
        attributes: Box<[ModuleAttribute]>,
        imported: ModuleExportName,
        binding: BindingId,
    },
}

impl ModuleImport {
    /// Creates a static import with no local bindings.
    pub fn bare(
        location: LocationId,
        source: impl Into<Box<str>>,
        attributes: impl Into<Box<[ModuleAttribute]>>,
    ) -> Self {
        Self::Bare {
            location,
            source: source.into(),
            attributes: attributes.into(),
        }
    }

    /// Creates a default import.
    pub fn default(
        location: LocationId,
        source: impl Into<Box<str>>,
        attributes: impl Into<Box<[ModuleAttribute]>>,
        binding: BindingId,
    ) -> Self {
        Self::Default {
            location,
            source: source.into(),
            attributes: attributes.into(),
            binding,
        }
    }

    /// Creates a namespace import.
    pub fn namespace(
        location: LocationId,
        source: impl Into<Box<str>>,
        attributes: impl Into<Box<[ModuleAttribute]>>,
        binding: BindingId,
    ) -> Self {
        Self::Namespace {
            location,
            source: source.into(),
            attributes: attributes.into(),
            binding,
        }
    }

    /// Creates a named import.
    pub fn named(
        location: LocationId,
        source: impl Into<Box<str>>,
        attributes: impl Into<Box<[ModuleAttribute]>>,
        imported: ModuleExportName,
        binding: BindingId,
    ) -> Self {
        Self::Named {
            location,
            source: source.into(),
            attributes: attributes.into(),
            imported,
            binding,
        }
    }

    /// Returns the source location that introduced this import.
    pub const fn location(&self) -> LocationId {
        match self {
            Self::Bare { location, .. }
            | Self::Default { location, .. }
            | Self::Namespace { location, .. }
            | Self::Named { location, .. } => *location,
        }
    }

    /// Returns the imported module specifier.
    pub fn source(&self) -> &str {
        match self {
            Self::Bare { source, .. }
            | Self::Default { source, .. }
            | Self::Namespace { source, .. }
            | Self::Named { source, .. } => source,
        }
    }

    /// Returns static import attributes in source order.
    pub fn attributes(&self) -> &[ModuleAttribute] {
        match self {
            Self::Bare { attributes, .. }
            | Self::Default { attributes, .. }
            | Self::Namespace { attributes, .. }
            | Self::Named { attributes, .. } => attributes,
        }
    }

    /// Returns the local binding created by this import, if any.
    pub const fn binding(&self) -> Option<BindingId> {
        match self {
            Self::Bare { .. } => None,
            Self::Default { binding, .. }
            | Self::Namespace { binding, .. }
            | Self::Named { binding, .. } => Some(*binding),
        }
    }

    /// Returns the source module name selected by a named import.
    pub const fn imported_name(&self) -> Option<&ModuleExportName> {
        match self {
            Self::Named { imported, .. } => Some(imported),
            Self::Bare { .. } | Self::Default { .. } | Self::Namespace { .. } => None,
        }
    }
}
