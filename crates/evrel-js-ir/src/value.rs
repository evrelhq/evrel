//! Values flowing between IR operations and blocks.

use crate::{BlockId, OperationId};

/// Metadata stored for an IR value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueData {
    definition: ValueDefinition,
    uses: Vec<ValueUse>,
}

impl ValueData {
    pub(crate) fn new(definition: ValueDefinition) -> Self {
        Self {
            definition,
            uses: Vec::new(),
        }
    }

    /// Returns where the value is defined.
    pub fn definition(&self) -> &ValueDefinition {
        &self.definition
    }

    /// Returns every operand slot that reads the value.
    pub fn uses(&self) -> &[ValueUse] {
        &self.uses
    }

    pub(crate) fn add_use(&mut self, use_site: ValueUse) {
        assert!(
            !self.uses.contains(&use_site),
            "value already contains use site {use_site:?}"
        );

        self.uses.push(use_site);
    }

    pub(crate) fn remove_use(&mut self, use_site: ValueUse) {
        let index = self
            .uses
            .iter()
            .position(|current| *current == use_site)
            .expect("value does not contain the requested use site");

        self.uses.remove(index);
    }

    pub(crate) fn set_block_parameter_index(&mut self, parameter_index: u32) {
        let ValueDefinition::BlockParameter {
            parameter_index: current,
            ..
        } = &mut self.definition
        else {
            panic!("only block parameters have a block parameter index");
        };

        *current = parameter_index;
    }
}

/// Describes where an IR value originates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueDefinition {
    /// A value received at a JavaScript function boundary.
    FunctionParameter {
        /// The value's position in the function's parameter list.
        parameter_index: u32,
    },

    /// A result produced by an operation.
    OperationResult {
        /// The defining operation.
        operation: OperationId,

        /// The value's position in the operation's result list.
        result_index: u32,
    },

    /// A parameter received when control enters a block.
    BlockParameter {
        /// The block receiving the value.
        block: BlockId,

        /// The value's position in the block's parameter list.
        parameter_index: u32,
    },
}

/// Identifies one operation operand that reads a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueUse {
    operation: OperationId,
    operand_index: u32,
}

impl ValueUse {
    pub(crate) fn new(operation: OperationId, operand_index: u32) -> Self {
        Self {
            operation,
            operand_index,
        }
    }

    /// Returns the operation containing the operand.
    pub fn operation(self) -> OperationId {
        self.operation
    }

    /// Returns the operand's position within the operation.
    pub fn operand_index(self) -> u32 {
        self.operand_index
    }
}

#[cfg(test)]
mod tests {
    use crate::OperationId;

    use super::{ValueData, ValueDefinition, ValueUse};

    #[test]
    fn tracks_distinct_operand_uses() {
        let definer = OperationId::from_index(0);
        let user = OperationId::from_index(1);

        let mut value = ValueData::new(ValueDefinition::OperationResult {
            operation: definer,
            result_index: 0,
        });

        value.add_use(ValueUse::new(user, 0));
        value.add_use(ValueUse::new(user, 1));

        assert_eq!(value.uses().len(), 2);
    }
}
