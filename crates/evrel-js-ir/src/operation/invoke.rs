//! Explicit exceptional control flow for one ordinary operation.

use crate::{BlockParameterSource, ValueType};

use super::{BlockTarget, OperationKind, OperationSuccessor};

/// Executes one ordinary operation with explicit normal and exceptional
/// continuations.
///
/// The contained operation is behavior owned by this terminator, not a
/// separately scheduled IR operation.
#[derive(Debug, Clone, PartialEq)]
pub struct InvokeOp {
    operation: OperationKind,
    normal_target: BlockTarget,
    exception_target: BlockTarget,
}

impl InvokeOp {
    /// Creates explicit normal and exceptional continuations.
    pub fn new(
        operation: OperationKind,
        normal_target: BlockTarget,
        exception_target: BlockTarget,
    ) -> Self {
        assert!(
            !operation.is_terminator(),
            "invoke must contain an ordinary operation"
        );

        Self {
            operation,
            normal_target,
            exception_target,
        }
    }

    /// Returns the ordinary operation executed by this terminator.
    pub const fn operation(&self) -> &OperationKind {
        &self.operation
    }

    /// Returns the continuation entered when the operation succeeds.
    pub const fn normal_target(&self) -> BlockTarget {
        self.normal_target
    }

    /// Returns the continuation entered when the operation throws.
    pub const fn exception_target(&self) -> BlockTarget {
        self.exception_target
    }

    pub(crate) fn successors(&self) -> [OperationSuccessor; 2] {
        let operation_operand_count = self.operation.operand_count();

        [
            OperationSuccessor::new(self.normal_target, operation_operand_count)
                .with_produced_arguments(self.operation.result_count()),
            OperationSuccessor::new(
                self.exception_target,
                operation_operand_count + self.normal_target.argument_count(),
            )
            .with_produced_arguments(1),
        ]
    }

    pub(crate) const fn produced_argument_source(
        &self,
        successor_index: usize,
    ) -> BlockParameterSource {
        match successor_index {
            0 => BlockParameterSource::Produced,
            1 => BlockParameterSource::Exception,
            _ => panic!("invoke has no successor at this index"),
        }
    }

    pub(crate) fn produced_argument_type(
        &self,
        successor_index: usize,
        produced_index: usize,
    ) -> Option<ValueType> {
        match successor_index {
            0 if produced_index < self.operation.result_count() => Some(ValueType::JsValue),
            1 if produced_index == 0 => Some(ValueType::JsValue),
            _ => None,
        }
    }

    pub(crate) fn target_mut(&mut self, successor_index: usize) -> &mut BlockTarget {
        match successor_index {
            0 => &mut self.normal_target,
            1 => &mut self.exception_target,
            _ => panic!("invoke has no successor {successor_index}"),
        }
    }

    pub(crate) fn operand_count(&self) -> usize {
        self.operation.operand_count()
            + self.normal_target.argument_count()
            + self.exception_target.argument_count()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        BlockId, BlockParameterSource, BlockTarget, LoadPropertyOp, OperationKind, PropertyKey,
        ValueType,
    };

    use super::InvokeOp;

    #[test]
    fn lays_out_operation_and_successor_operands() {
        let normal = BlockTarget::new(BlockId::from_index(1), 2);
        let exception = BlockTarget::new(BlockId::from_index(2), 1);
        let operation = OperationKind::LoadProperty(LoadPropertyOp::new(PropertyKey::Computed));
        let invoke = InvokeOp::new(operation, normal, exception);
        let [normal, exception] = invoke.successors();

        assert_eq!(normal.argument_operand_range(), 2..4);
        assert_eq!(normal.produced_argument_count(), 1);
        assert_eq!(exception.argument_operand_range(), 4..5);
        assert_eq!(exception.produced_argument_count(), 1);
        assert_eq!(invoke.operand_count(), 5);
        assert_eq!(
            invoke.produced_argument_source(0),
            BlockParameterSource::Produced,
        );
        assert_eq!(
            invoke.produced_argument_source(1),
            BlockParameterSource::Exception,
        );
        assert_eq!(
            invoke.produced_argument_type(0, 0),
            Some(ValueType::JsValue),
        );
        assert_eq!(
            invoke.produced_argument_type(1, 0),
            Some(ValueType::JsValue),
        );
    }
}
