//! JavaScript private-name metadata.

use crate::{PrivateNameId, arena::Arena};

/// Metadata for one lexically scoped JavaScript private name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PrivateNameData {
    name: Box<str>,
}

impl PrivateNameData {
    /// Creates private-name metadata.
    pub fn new(name: impl Into<Box<str>>) -> Self {
        Self { name: name.into() }
    }

    /// Returns the private name without its leading `#`.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Owns the canonical private names declared within one module.
pub(crate) struct PrivateNameTable {
    names: Arena<PrivateNameId, PrivateNameData>,
}

impl PrivateNameTable {
    pub(crate) fn new() -> Self {
        Self {
            names: Arena::new(),
        }
    }

    pub(crate) const fn len(&self) -> usize {
        self.names.len()
    }

    pub(crate) fn get(&self, id: PrivateNameId) -> Option<&PrivateNameData> {
        self.names.get(id)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (PrivateNameId, &PrivateNameData)> + '_ {
        self.names.iter()
    }

    pub(crate) fn create(&mut self, name: impl Into<Box<str>>) -> PrivateNameId {
        self.names.alloc(PrivateNameData::new(name))
    }
}

impl Default for PrivateNameTable {
    fn default() -> Self {
        Self::new()
    }
}
