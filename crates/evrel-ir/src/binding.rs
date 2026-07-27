//! JavaScript binding metadata.

use crate::{BindingId, FunctionId, arena::Arena};

/// Describes how a JavaScript binding was declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingKind {
    /// A lexical immutable binding.
    Const,

    /// A lexical mutable binding.
    Let,

    /// A lexical binding initialized during class evaluation.
    Class,

    /// A function-scoped mutable binding.
    Var,

    /// A binding initialized to a hoisted function object.
    Function,

    /// An immutable live binding supplied by module instantiation.
    Import,

    /// A binding initialized from a function argument.
    Parameter,

    /// A mutable lexical binding initialized from a caught exception.
    Catch,
}

/// Metadata for one JavaScript binding.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BindingData {
    declaring_function: FunctionId,
    name: Box<str>,
    kind: BindingKind,
}

impl BindingData {
    /// Creates binding metadata.
    pub fn new(
        declaring_function: FunctionId,
        name: impl Into<Box<str>>,
        kind: BindingKind,
    ) -> Self {
        Self {
            declaring_function,
            name: name.into(),
            kind,
        }
    }

    /// Returns the function whose lexical environment declares the binding.
    pub const fn declaring_function(&self) -> FunctionId {
        self.declaring_function
    }

    /// Returns the binding's source-level name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns how the binding was declared.
    pub const fn kind(&self) -> BindingKind {
        self.kind
    }
}

/// Owns the canonical bindings declared within one module.
pub(crate) struct BindingTable {
    bindings: Arena<BindingId, BindingData>,
}

impl BindingTable {
    pub(crate) fn new() -> Self {
        Self {
            bindings: Arena::new(),
        }
    }

    pub(crate) const fn len(&self) -> usize {
        self.bindings.len()
    }

    pub(crate) fn get(&self, id: BindingId) -> Option<&BindingData> {
        self.bindings.get(id)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (BindingId, &BindingData)> + '_ {
        self.bindings.iter()
    }

    pub(crate) fn create(
        &mut self,
        declaring_function: FunctionId,
        name: impl Into<Box<str>>,
        kind: BindingKind,
    ) -> BindingId {
        self.bindings
            .alloc(BindingData::new(declaring_function, name, kind))
    }
}

impl Default for BindingTable {
    fn default() -> Self {
        Self::new()
    }
}
