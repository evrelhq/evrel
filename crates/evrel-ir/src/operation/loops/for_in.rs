//! JavaScript `for...in` enumeration.

use crate::BindingId;

use super::super::{BlockTarget, OperationEffects, OperationSuccessor};

/// Enumerates the enumerable string keys of a JavaScript value.
///
/// The object is operand zero. Each execution either transfers to the body
/// target and produces one property-key parameter, or transfers to the exit
/// target when enumeration is complete. Re-entering this operation advances
/// the same source-level enumeration.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForInOp {
    body_target: BlockTarget,
    exit_target: BlockTarget,
    per_iteration_bindings: Box<[BindingId]>,
    labels: Box<[Box<str>]>,
}

impl ForInOp {
    /// Creates a `for...in` loop header.
    pub fn new(
        body_target: BlockTarget,
        exit_target: BlockTarget,
        per_iteration_bindings: Box<[BindingId]>,
        labels: Box<[Box<str>]>,
    ) -> Self {
        Self {
            body_target,
            exit_target,
            per_iteration_bindings,
            labels,
        }
    }

    /// Returns the target receiving the next property key.
    pub const fn body_target(&self) -> BlockTarget {
        self.body_target
    }

    /// Returns the target used when enumeration is complete.
    pub const fn exit_target(&self) -> BlockTarget {
        self.exit_target
    }

    /// Returns header bindings with fresh instances for each iteration.
    pub fn per_iteration_bindings(&self) -> &[BindingId] {
        &self.per_iteration_bindings
    }

    /// Returns source labels in outermost-to-innermost order.
    pub fn labels(&self) -> &[Box<str>] {
        &self.labels
    }

    /// Returns the operation's executable successors.
    pub fn successors(&self) -> Vec<OperationSuccessor> {
        vec![
            OperationSuccessor::new(self.body_target, 1).with_produced_arguments(1),
            OperationSuccessor::new(self.exit_target, 1 + self.body_target.argument_count()),
        ]
    }

    pub(crate) const fn effects(&self) -> OperationEffects {
        OperationEffects::MAY_THROW_AND_OBSERVABLE
    }

    pub(crate) const fn operand_count(&self) -> usize {
        1 + self.body_target.argument_count() + self.exit_target.argument_count()
    }

    pub(crate) const fn result_count(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use crate::{BlockId, BlockTarget};

    use super::ForInOp;

    #[test]
    fn produces_one_property_key_for_the_body_edge() {
        let body = BlockTarget::new(BlockId::from_index(1), 0);
        let exit = BlockTarget::new(BlockId::from_index(2), 0);
        let operation = ForInOp::new(body, exit, Box::new([]), Box::new([]));
        let successors = operation.successors();

        assert_eq!(operation.body_target(), body);
        assert_eq!(operation.exit_target(), exit);
        assert_eq!(successors[0].target(), body);
        assert_eq!(successors[0].produced_argument_count(), 1);
        assert_eq!(successors[1].target(), exit);
        assert_eq!(successors[1].produced_argument_count(), 0);
        assert_eq!(operation.operand_count(), 1);
        assert_eq!(operation.result_count(), 0);
    }
}
