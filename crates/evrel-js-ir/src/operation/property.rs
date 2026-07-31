//! JavaScript property operations.

use crate::PrivateNameId;

/// Describes how a JavaScript property key is supplied.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PropertyKey {
    /// A property name known during lowering.
    Static(Box<str>),

    /// A raw JavaScript key value supplied as an operation operand.
    Computed,

    /// A lexically resolved class-private name.
    Private(PrivateNameId),
}

/// Describes a non-private property key used through `super`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SuperPropertyKey {
    Static(Box<str>),
    Computed,
}

/// Reads a property from a JavaScript value.
///
/// The first operand is always the object. A computed property additionally
/// consumes its raw JavaScript key value as the second operand. Converting that
/// value to a property key is part of the property read's semantics.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoadPropertyOp {
    key: PropertyKey,
}

impl LoadPropertyOp {
    /// Creates a property-read operation.
    pub const fn new(key: PropertyKey) -> Self {
        Self { key }
    }

    /// Returns how the property key is supplied.
    pub const fn key(&self) -> &PropertyKey {
        &self.key
    }

    pub(crate) const fn operand_count(&self) -> usize {
        match &self.key {
            PropertyKey::Static(_) | PropertyKey::Private(_) => 1,
            PropertyKey::Computed => 2,
        }
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

/// Reads a property using ECMAScript `super` semantics.
///
/// The home object and receiver come from the enclosing method context. A
/// raw computed key value is supplied as the operation's only operand.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoadSuperPropertyOp {
    key: SuperPropertyKey,
}

impl LoadSuperPropertyOp {
    pub const fn new(key: SuperPropertyKey) -> Self {
        Self { key }
    }

    pub const fn key(&self) -> &SuperPropertyKey {
        &self.key
    }

    pub(crate) const fn operand_count(&self) -> usize {
        match self.key {
            SuperPropertyKey::Static(_) => 0,
            SuperPropertyKey::Computed => 1,
        }
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

/// Writes a value to a JavaScript property.
///
/// Operands are ordered as:
///
/// - static: object, value
/// - computed: object, raw key value, value
///
/// Converting a computed key is part of the property write's semantics.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StorePropertyOp {
    key: PropertyKey,
}

impl StorePropertyOp {
    /// Creates a property-write operation.
    pub const fn new(key: PropertyKey) -> Self {
        Self { key }
    }

    /// Returns how the property key is supplied.
    pub const fn key(&self) -> &PropertyKey {
        &self.key
    }

    pub(crate) const fn operand_count(&self) -> usize {
        match &self.key {
            PropertyKey::Static(_) | PropertyKey::Private(_) => 2,
            PropertyKey::Computed => 3,
        }
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }
}

/// Writes a property using ECMAScript `super` semantics.
///
/// Operand layouts:
///
/// - static: value
/// - computed: raw key value, value
///
/// Converting a computed key is part of the property write's semantics.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StoreSuperPropertyOp {
    key: SuperPropertyKey,
}

impl StoreSuperPropertyOp {
    pub const fn new(key: SuperPropertyKey) -> Self {
        Self { key }
    }

    pub const fn key(&self) -> &SuperPropertyKey {
        &self.key
    }

    pub(crate) const fn operand_count(&self) -> usize {
        match self.key {
            SuperPropertyKey::Static(_) => 1,
            SuperPropertyKey::Computed => 2,
        }
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use crate::{JsModuleIr, ModuleBuilder};

    use super::{
        LoadPropertyOp, LoadSuperPropertyOp, PropertyKey, StorePropertyOp, StoreSuperPropertyOp,
        SuperPropertyKey,
    };

    #[test]
    fn static_property_reads_consume_only_the_object() {
        let operation = LoadPropertyOp::new(PropertyKey::Static("log".into()));

        assert_eq!(operation.key(), &PropertyKey::Static("log".into()));
        assert_eq!(operation.operand_count(), 1);
        assert_eq!(operation.result_count(), 1);
    }

    #[test]
    fn super_property_reads_use_the_implicit_method_context() {
        let static_operation = LoadSuperPropertyOp::new(SuperPropertyKey::Static("value".into()));
        let computed_operation = LoadSuperPropertyOp::new(SuperPropertyKey::Computed);

        assert_eq!(static_operation.operand_count(), 0);
        assert_eq!(computed_operation.operand_count(), 1);
        assert_eq!(static_operation.result_count(), 1);
        assert_eq!(computed_operation.result_count(), 1);
    }

    #[test]
    fn super_property_stores_use_the_implicit_method_context() {
        let static_operation = StoreSuperPropertyOp::new(SuperPropertyKey::Static("value".into()));
        let computed_operation = StoreSuperPropertyOp::new(SuperPropertyKey::Computed);

        assert_eq!(static_operation.operand_count(), 1);
        assert_eq!(computed_operation.operand_count(), 2);
        assert_eq!(static_operation.result_count(), 0);
        assert_eq!(computed_operation.result_count(), 0);
    }

    #[test]
    fn computed_property_reads_also_consume_the_key() {
        let operation = LoadPropertyOp::new(PropertyKey::Computed);

        assert_eq!(operation.key(), &PropertyKey::Computed);
        assert_eq!(operation.operand_count(), 2);
        assert_eq!(operation.result_count(), 1);
    }

    #[test]
    fn static_property_stores_consume_object_and_value() {
        let operation = StorePropertyOp::new(PropertyKey::Static("value".into()));

        assert_eq!(operation.operand_count(), 2);
        assert_eq!(operation.result_count(), 0);
    }

    #[test]
    fn computed_property_stores_also_consume_the_key() {
        let operation = StorePropertyOp::new(PropertyKey::Computed);

        assert_eq!(operation.operand_count(), 3);
        assert_eq!(operation.result_count(), 0);
    }

    #[test]
    fn private_property_operations_use_no_runtime_key_operand() {
        let mut module = JsModuleIr::new();
        let private_name = ModuleBuilder::new(&mut module).create_private_name("value");

        let load = LoadPropertyOp::new(PropertyKey::Private(private_name));
        let store = StorePropertyOp::new(PropertyKey::Private(private_name));

        assert_eq!(load.operand_count(), 1);
        assert_eq!(store.operand_count(), 2);
    }
}
