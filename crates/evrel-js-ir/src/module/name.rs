//! Names used by JavaScript module linkage.

/// An imported or exported module binding name.
///
/// Module names are strings semantically, while source syntax distinguishes
/// identifier spellings from quoted string spellings.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModuleExportName {
    Identifier(Box<str>),
    String(Box<str>),
}

impl ModuleExportName {
    /// Returns the semantic module name.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Identifier(name) | Self::String(name) => name,
        }
    }
}

/// One static module import or export attribute.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleAttribute {
    key: ModuleExportName,
    value: Box<str>,
}

impl ModuleAttribute {
    /// Creates a module attribute.
    pub fn new(key: ModuleExportName, value: impl Into<Box<str>>) -> Self {
        Self {
            key,
            value: value.into(),
        }
    }

    /// Returns the attribute key.
    pub const fn key(&self) -> &ModuleExportName {
        &self.key
    }

    /// Returns the attribute value.
    pub fn value(&self) -> &str {
        &self.value
    }
}
