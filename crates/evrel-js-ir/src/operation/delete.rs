//! JavaScript delete operations.

use super::{OperationEffects, PropertyKey};

/// The kind of target supplied to JavaScript `delete`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DeleteTarget {
    /// A non-reference value.
    ///
    /// JavaScript evaluates the value and then produces `true`.
    Value,

    /// A property reference.
    Property(PropertyKey),
}

/// Applies JavaScript `delete` semantics.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeleteOp {
    target: DeleteTarget,
}

impl DeleteOp {
    /// Creates a delete operation.
    pub const fn new(target: DeleteTarget) -> Self {
        assert!(
            !matches!(&target, DeleteTarget::Property(PropertyKey::Private(_))),
            "private properties cannot be deleted",
        );

        Self { target }
    }

    /// Returns the target being deleted.
    pub const fn target(&self) -> &DeleteTarget {
        &self.target
    }

    /// Returns the observable effects of deleting this target.
    pub const fn effects(&self) -> OperationEffects {
        match &self.target {
            DeleteTarget::Value => OperationEffects::NONE,
            DeleteTarget::Property(_) => OperationEffects::MAY_THROW,
        }
    }

    pub(crate) const fn operand_count(&self) -> usize {
        match &self.target {
            DeleteTarget::Value => 1,
            DeleteTarget::Property(PropertyKey::Static(_)) => 1,
            DeleteTarget::Property(PropertyKey::Computed) => 2,
            DeleteTarget::Property(PropertyKey::Private(_)) => {
                panic!("private properties cannot be deleted")
            }
        }
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

#[cfg(test)]
mod tests {
    use crate::PropertyKey;

    use super::{DeleteOp, DeleteTarget};

    #[test]
    fn property_delete_shapes_follow_the_property_key() {
        let static_delete =
            DeleteOp::new(DeleteTarget::Property(PropertyKey::Static("value".into())));
        let computed_delete = DeleteOp::new(DeleteTarget::Property(PropertyKey::Computed));

        assert_eq!(static_delete.operand_count(), 1);
        assert_eq!(computed_delete.operand_count(), 2);
        assert_eq!(static_delete.result_count(), 1);
    }

    #[test]
    fn value_delete_consumes_the_evaluated_value() {
        let operation = DeleteOp::new(DeleteTarget::Value);

        assert_eq!(operation.operand_count(), 1);
        assert_eq!(operation.result_count(), 1);
    }

    #[test]
    fn classifies_delete_throw_behavior() {
        let value = DeleteOp::new(DeleteTarget::Value);
        let property = DeleteOp::new(DeleteTarget::Property(PropertyKey::Computed));

        assert!(!value.effects().may_throw());
        assert!(property.effects().may_throw());
    }

    #[test]
    #[should_panic(expected = "private properties cannot be deleted")]
    fn rejects_private_property_deletion() {
        let mut module = crate::JsModuleIr::new();
        let private_name = crate::ModuleBuilder::new(&mut module).create_private_name("value");

        DeleteOp::new(DeleteTarget::Property(PropertyKey::Private(private_name)));
    }
}
