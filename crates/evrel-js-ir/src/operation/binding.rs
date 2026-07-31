//! JavaScript binding operations.

use crate::BindingId;

use super::OperationEffects;

/// Initializes a binding with its first runtime value.
///
/// The value is the operation's only operand.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InitializeBindingOp {
    binding: BindingId,
}

impl InitializeBindingOp {
    /// Creates a binding-initialization operation.
    pub const fn new(binding: BindingId) -> Self {
        Self { binding }
    }

    /// Returns the binding being initialized.
    pub const fn binding(&self) -> BindingId {
        self.binding
    }

    /// Returns the effects of initializing the binding's environment cell.
    pub const fn effects(&self) -> OperationEffects {
        OperationEffects::OBSERVABLE
    }

    pub(crate) const fn operand_count(&self) -> usize {
        1
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }
}

/// Reads the current value of a binding.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoadBindingOp {
    binding: BindingId,
}

impl LoadBindingOp {
    /// Creates a binding-read operation.
    pub const fn new(binding: BindingId) -> Self {
        Self { binding }
    }

    /// Returns the binding being read.
    pub const fn binding(&self) -> BindingId {
        self.binding
    }

    /// Returns the observable effects of reading the binding.
    pub const fn effects(&self) -> OperationEffects {
        // The binding may still be in its temporal dead zone.
        OperationEffects::MAY_THROW
    }

    pub(crate) const fn operand_count(&self) -> usize {
        0
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

/// Assigns a new value to an initialized binding.
///
/// The value is the operation's only operand.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StoreBindingOp {
    binding: BindingId,
}

impl StoreBindingOp {
    /// Creates a binding-store operation.
    pub const fn new(binding: BindingId) -> Self {
        Self { binding }
    }

    /// Returns the binding being written.
    pub const fn binding(&self) -> BindingId {
        self.binding
    }

    /// Returns the observable effects of assigning to the binding.
    pub const fn effects(&self) -> OperationEffects {
        // Assignment may target an immutable or uninitialized binding.
        OperationEffects::MAY_THROW_AND_OBSERVABLE
    }

    pub(crate) const fn operand_count(&self) -> usize {
        1
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use crate::BindingId;

    use super::{InitializeBindingOp, LoadBindingOp, StoreBindingOp};

    #[test]
    fn defines_binding_operation_shapes() {
        let binding = BindingId::from_index(3);
        let initialize = InitializeBindingOp::new(binding);
        let load = LoadBindingOp::new(binding);
        let store = StoreBindingOp::new(binding);

        assert_eq!(initialize.binding(), binding);
        assert_eq!(initialize.operand_count(), 1);
        assert_eq!(initialize.result_count(), 0);

        assert_eq!(load.binding(), binding);
        assert_eq!(load.operand_count(), 0);
        assert_eq!(load.result_count(), 1);

        assert_eq!(store.binding(), binding);
        assert_eq!(store.operand_count(), 1);
        assert_eq!(store.result_count(), 0);
    }

    #[test]
    fn classifies_binding_throw_behavior() {
        let binding = BindingId::from_index(3);
        let initialize = InitializeBindingOp::new(binding);
        let load = LoadBindingOp::new(binding);
        let store = StoreBindingOp::new(binding);

        assert!(!initialize.effects().may_throw());
        assert!(initialize.effects().may_have_observable_effects());
        assert!(load.effects().may_throw());
        assert!(store.effects().may_throw());
        assert!(store.effects().may_have_observable_effects());
    }
}
