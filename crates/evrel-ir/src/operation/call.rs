//! JavaScript invocation operations.

use crate::{PropertyKey, RegionId, SuperPropertyKey};

/// Describes whether a value call supplies an explicit JavaScript `this`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallReceiver {
    /// The call has no receiver operand.
    None,

    /// The call's second operand is its receiver.
    Explicit,
}

/// Describes how a call resolves its callable value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CallTarget {
    /// Operands are `[callee]` or `[callee, receiver]`.
    Value { receiver: CallReceiver },

    /// Operands are `[object]` or `[object, computed_key]`.
    Property(PropertyKey),

    /// Operands are `[]` or `[computed_key]`.
    SuperProperty(SuperPropertyKey),
}

impl CallTarget {
    /// Returns the number of operands used to resolve the callable.
    pub const fn operand_count(&self) -> usize {
        match self {
            Self::Value {
                receiver: CallReceiver::None,
            } => 1,
            Self::Value {
                receiver: CallReceiver::Explicit,
            } => 2,
            Self::Property(PropertyKey::Static(_) | PropertyKey::Private(_)) => 1,
            Self::Property(PropertyKey::Computed) => 2,
            Self::SuperProperty(SuperPropertyKey::Static(_)) => 0,
            Self::SuperProperty(SuperPropertyKey::Computed) => 1,
        }
    }
}

/// One source-ordered JavaScript invocation argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallArgument {
    /// Evaluates one expression and passes its value.
    Value { expression: RegionId },

    /// Evaluates one expression and expands its iterator.
    Spread { expression: RegionId },
}

impl CallArgument {
    /// Returns the expression region evaluated for this argument.
    pub const fn expression(self) -> RegionId {
        match self {
            Self::Value { expression } | Self::Spread { expression } => expression,
        }
    }
}

/// Invokes a JavaScript callable.
///
/// Target operands are evaluated first. Property targets are resolved next.
/// Argument regions then execute from left to right before invocation. Spread
/// iteration completes before evaluation advances to the next argument.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallOp {
    target: CallTarget,
    arguments: Box<[CallArgument]>,
    has_pure_annotation: bool,
}

impl CallOp {
    /// Creates a call with source-ordered argument regions.
    pub fn new(target: CallTarget, arguments: impl Into<Box<[CallArgument]>>) -> Self {
        Self {
            target,
            arguments: arguments.into(),
            has_pure_annotation: false,
        }
    }

    /// Marks whether this invocation carried a `/* @__PURE__ */` annotation.
    #[must_use]
    pub const fn with_pure_annotation(mut self, has_pure_annotation: bool) -> Self {
        self.has_pure_annotation = has_pure_annotation;
        self
    }

    /// Returns whether this invocation carried a `/* @__PURE__ */` annotation.
    pub const fn has_pure_annotation(&self) -> bool {
        self.has_pure_annotation
    }

    /// Returns how the callable value is resolved.
    pub const fn target(&self) -> &CallTarget {
        &self.target
    }

    /// Returns arguments in source order.
    pub fn arguments(&self) -> &[CallArgument] {
        &self.arguments
    }

    /// Returns argument regions in semantic evaluation order.
    pub fn regions(&self) -> Vec<RegionId> {
        self.arguments
            .iter()
            .map(|argument| argument.expression())
            .collect()
    }

    /// Returns the callee's operand position for a value call.
    pub const fn callee_operand_index(&self) -> Option<usize> {
        match self.target {
            CallTarget::Value { .. } => Some(0),
            CallTarget::Property(_) | CallTarget::SuperProperty(_) => None,
        }
    }

    /// Returns the receiver's operand position, when present.
    pub const fn receiver_operand_index(&self) -> Option<usize> {
        match self.target {
            CallTarget::Value {
                receiver: CallReceiver::Explicit,
            } => Some(1),
            CallTarget::Property(_) => Some(0),
            CallTarget::Value {
                receiver: CallReceiver::None,
            }
            | CallTarget::SuperProperty(_) => None,
        }
    }

    /// Returns the computed ordinary-property key's operand position.
    pub const fn property_key_operand_index(&self) -> Option<usize> {
        match self.target {
            CallTarget::Property(PropertyKey::Computed) => Some(1),
            _ => None,
        }
    }

    /// Returns the computed `super` key's operand position.
    pub const fn super_property_key_operand_index(&self) -> Option<usize> {
        match self.target {
            CallTarget::SuperProperty(SuperPropertyKey::Computed) => Some(0),
            _ => None,
        }
    }

    pub(crate) const fn operand_count(&self) -> usize {
        self.target.operand_count()
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

/// Calls the implicit superclass constructor.
///
/// The superclass constructor and `new.target` come from the enclosing
/// derived-constructor context. Argument regions execute from left to right.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SuperCallOp {
    arguments: Box<[CallArgument]>,
    has_pure_annotation: bool,
}

impl SuperCallOp {
    pub fn new(arguments: impl Into<Box<[CallArgument]>>) -> Self {
        Self {
            arguments: arguments.into(),
            has_pure_annotation: false,
        }
    }

    /// Marks whether this invocation carried a `/* @__PURE__ */` annotation.
    #[must_use]
    pub const fn with_pure_annotation(mut self, has_pure_annotation: bool) -> Self {
        self.has_pure_annotation = has_pure_annotation;
        self
    }

    /// Returns whether this invocation carried a `/* @__PURE__ */` annotation.
    pub const fn has_pure_annotation(&self) -> bool {
        self.has_pure_annotation
    }

    pub fn arguments(&self) -> &[CallArgument] {
        &self.arguments
    }

    pub fn regions(&self) -> Vec<RegionId> {
        self.arguments
            .iter()
            .map(|argument| argument.expression())
            .collect()
    }

    pub(crate) const fn operand_count(&self) -> usize {
        0
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

/// Constructs a JavaScript object using a constructor value.
///
/// The constructor is the operation's only operand. Argument regions execute
/// from left to right after the constructor has been evaluated.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConstructOp {
    arguments: Box<[CallArgument]>,
    has_pure_annotation: bool,
}

impl ConstructOp {
    pub fn new(arguments: impl Into<Box<[CallArgument]>>) -> Self {
        Self {
            arguments: arguments.into(),
            has_pure_annotation: false,
        }
    }

    /// Marks whether this invocation carried a `/* @__PURE__ */` annotation.
    #[must_use]
    pub const fn with_pure_annotation(mut self, has_pure_annotation: bool) -> Self {
        self.has_pure_annotation = has_pure_annotation;
        self
    }

    /// Returns whether this invocation carried a `/* @__PURE__ */` annotation.
    pub const fn has_pure_annotation(&self) -> bool {
        self.has_pure_annotation
    }

    pub fn arguments(&self) -> &[CallArgument] {
        &self.arguments
    }

    pub fn regions(&self) -> Vec<RegionId> {
        self.arguments
            .iter()
            .map(|argument| argument.expression())
            .collect()
    }

    /// Returns the constructor's operand position.
    pub const fn constructor_operand_index(&self) -> usize {
        0
    }

    pub(crate) const fn operand_count(&self) -> usize {
        1
    }

    pub(crate) const fn result_count(&self) -> usize {
        1
    }
}

#[cfg(test)]
mod tests {
    use crate::{PrivateNameId, PropertyKey, RegionId, SuperPropertyKey};

    use super::{CallArgument, CallOp, CallReceiver, CallTarget, ConstructOp, SuperCallOp};

    #[test]
    fn lays_out_value_and_property_targets() {
        let value = CallOp::new(
            CallTarget::Value {
                receiver: CallReceiver::Explicit,
            },
            [],
        );
        let static_property = CallOp::new(
            CallTarget::Property(PropertyKey::Static("method".into())),
            [],
        );
        let computed_property = CallOp::new(CallTarget::Property(PropertyKey::Computed), []);
        let private_property = CallOp::new(
            CallTarget::Property(PropertyKey::Private(PrivateNameId::from_index(0))),
            [],
        );

        assert_eq!(value.callee_operand_index(), Some(0));
        assert_eq!(value.receiver_operand_index(), Some(1));
        assert_eq!(value.operand_count(), 2);
        assert_eq!(static_property.receiver_operand_index(), Some(0));
        assert_eq!(static_property.operand_count(), 1);
        assert_eq!(computed_property.property_key_operand_index(), Some(1));
        assert_eq!(computed_property.operand_count(), 2);
        assert_eq!(private_property.receiver_operand_index(), Some(0));
        assert_eq!(private_property.operand_count(), 1);
    }

    #[test]
    fn preserves_call_argument_region_order() {
        let first = RegionId::from_index(1);
        let second = RegionId::from_index(2);
        let operation = CallOp::new(
            CallTarget::Value {
                receiver: CallReceiver::None,
            },
            [
                CallArgument::Value { expression: first },
                CallArgument::Spread { expression: second },
            ],
        );

        assert_eq!(operation.arguments().len(), 2);
        assert_eq!(operation.regions(), [first, second]);
        assert_eq!(operation.operand_count(), 1);
        assert_eq!(operation.result_count(), 1);
        assert!(!operation.has_pure_annotation());
        assert!(operation.with_pure_annotation(true).has_pure_annotation());
    }

    #[test]
    fn lays_out_super_property_targets() {
        let static_call = CallOp::new(
            CallTarget::SuperProperty(SuperPropertyKey::Static("method".into())),
            [],
        );
        let computed_call = CallOp::new(CallTarget::SuperProperty(SuperPropertyKey::Computed), []);

        assert_eq!(static_call.operand_count(), 0);
        assert_eq!(computed_call.super_property_key_operand_index(), Some(0));
        assert_eq!(computed_call.operand_count(), 1);
    }

    #[test]
    fn super_and_construct_arguments_are_regions() {
        let first = RegionId::from_index(1);
        let second = RegionId::from_index(2);
        let arguments = [
            CallArgument::Value { expression: first },
            CallArgument::Spread { expression: second },
        ];
        let super_call = SuperCallOp::new(arguments);
        let construct = ConstructOp::new(arguments);

        assert_eq!(super_call.regions(), [first, second]);
        assert_eq!(super_call.operand_count(), 0);
        assert!(!super_call.has_pure_annotation());
        assert!(super_call.with_pure_annotation(true).has_pure_annotation());
        assert_eq!(construct.regions(), [first, second]);
        assert_eq!(construct.constructor_operand_index(), 0);
        assert_eq!(construct.operand_count(), 1);
        assert!(!construct.has_pure_annotation());
        assert!(construct.with_pure_annotation(true).has_pure_annotation());
    }
}
