//! Invariant-preserving mutation of existing function IR.

use crate::{BlockId, FunctionIr, OperationId, ValueId};

/// Mutates an existing function while preserving IR invariants.
///
/// Initial construction belongs in `FunctionBuilder`. This editor exposes only
/// mutations currently required by middle-end transformations.
pub struct FunctionEditor<'ir> {
    ir: &'ir mut FunctionIr,
}

impl<'ir> FunctionEditor<'ir> {
    /// Creates an editor for an existing function.
    pub fn new(ir: &'ir mut FunctionIr) -> Self {
        Self { ir }
    }

    /// Returns the function currently being edited.
    pub fn ir(&self) -> &FunctionIr {
        &*self.ir
    }

    /// Removes ordinary operations whose results have no users outside the
    /// removal set.
    ///
    /// Terminators and operations owning inline regions cannot be removed
    /// through this narrow transformation API.
    pub fn remove_operations(&mut self, operations: impl IntoIterator<Item = OperationId>) {
        self.ir.remove_operations(operations);
    }

    /// Replaces every use of one value with another.
    ///
    /// Returns the number of rewritten operand positions.
    pub fn replace_all_uses(&mut self, value: ValueId, replacement: ValueId) -> usize {
        assert!(
            self.ir.value(replacement).is_some(),
            "replacement value must belong to the function",
        );

        let uses = self
            .ir
            .value(value)
            .expect("cannot replace an unknown value")
            .uses()
            .to_vec();

        if value == replacement {
            return 0;
        }

        for use_site in &uses {
            self.ir.replace_operand(
                use_site.operation(),
                use_site.operand_index() as usize,
                replacement,
            );
        }

        uses.len()
    }

    /// Appends one forwarded parameter to every selected block and updates all
    /// incoming edges.
    ///
    /// All parameters are allocated before edge arguments are requested,
    /// allowing cyclic merge blocks to forward new parameters.
    pub fn append_forwarded_block_parameters(
        &mut self,
        blocks: impl IntoIterator<Item = BlockId>,
        argument_for_edge: impl FnMut(
            &[(BlockId, ValueId)],
            BlockId,
            BlockId,
            OperationId,
            usize,
        ) -> ValueId,
    ) -> Vec<(BlockId, ValueId)> {
        self.ir
            .append_forwarded_block_parameters(blocks, argument_for_edge)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        BinaryOp, BinaryOperator, BlockParameterSource, BlockTarget, ConstantOp, ConstantValue,
        IfOp, JumpOp, ModuleBuilder, ModuleIr, OperationKind, UnwindTarget,
    };

    use super::FunctionEditor;

    #[test]
    fn replaces_all_uses_and_updates_use_lists() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (original, replacement, binary) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let original = builder.append_operation(
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(1.0))),
                [],
                UnwindTarget::Propagate,
            );
            let original = builder.operation_results(original)[0];
            let replacement = builder.append_operation(
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(2.0))),
                [],
                UnwindTarget::Propagate,
            );
            let replacement = builder.operation_results(replacement)[0];
            let binary = builder.append_operation(
                OperationKind::Binary(BinaryOp::new(BinaryOperator::StrictEqual)),
                [original, original],
                UnwindTarget::Propagate,
            );

            (original, replacement, binary)
        };

        let function = module.function_mut(function_id).unwrap();
        let mut editor = FunctionEditor::new(function);

        assert_eq!(editor.replace_all_uses(original, replacement), 2);
        assert_eq!(
            editor.ir().operation(binary).unwrap().operands(),
            [replacement, replacement],
        );
        assert!(editor.ir().value(original).unwrap().uses().is_empty());

        let replacement_uses = editor.ir().value(replacement).unwrap().uses();
        assert_eq!(replacement_uses.len(), 2);
        assert_eq!(replacement_uses[0].operation(), binary);
        assert_eq!(replacement_uses[0].operand_index(), 0);
        assert_eq!(replacement_uses[1].operation(), binary);
        assert_eq!(replacement_uses[1].operand_index(), 1);
    }

    #[test]
    fn removes_an_internally_connected_operation_batch() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (source, first, first_result, second, second_result) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);

            let source = builder.append_operation(
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(1.0))),
                [],
                UnwindTarget::Propagate,
            );
            let source = builder.operation_results(source)[0];

            let first = builder.append_operation(
                OperationKind::Binary(BinaryOp::new(BinaryOperator::StrictEqual)),
                [source, source],
                UnwindTarget::Propagate,
            );
            let first_result = builder.operation_results(first)[0];

            let second = builder.append_operation(
                OperationKind::Binary(BinaryOp::new(BinaryOperator::StrictEqual)),
                [first_result, first_result],
                UnwindTarget::Propagate,
            );
            let second_result = builder.operation_results(second)[0];

            (source, first, first_result, second, second_result)
        };

        let function = module.function_mut(function_id).unwrap();
        let mut editor = FunctionEditor::new(function);

        editor.remove_operations([first, second]);

        assert!(editor.ir().operation(first).is_none());
        assert!(editor.ir().operation(second).is_none());
        assert!(editor.ir().value(first_result).is_none());
        assert!(editor.ir().value(second_result).is_none());
        assert!(editor.ir().value(source).unwrap().uses().is_empty());
        assert_eq!(editor.ir().operation_count(), 1);
        assert_eq!(editor.ir().value_count(), 1);
    }

    #[test]
    fn appends_a_forwarded_parameter_to_every_incoming_edge() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (left, right, join, left_value, right_value, left_jump, right_jump) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let left = builder.create_block();
            let right = builder.create_block();
            let join = builder.create_block();

            let condition = builder.append_operation(
                OperationKind::Constant(ConstantOp::new(ConstantValue::Boolean(true))),
                [],
                UnwindTarget::Propagate,
            );
            let condition = builder.operation_results(condition)[0];
            builder.terminate(
                OperationKind::If(IfOp::new(
                    BlockTarget::new(left, 0),
                    BlockTarget::new(right, 0),
                    join,
                )),
                [condition],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(left);
            let left_value = builder.append_operation(
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(1.0))),
                [],
                UnwindTarget::Propagate,
            );
            let left_value = builder.operation_results(left_value)[0];
            let left_jump = builder.terminate(
                OperationKind::Jump(JumpOp::new(BlockTarget::new(join, 0))),
                [],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(right);
            let right_value = builder.append_operation(
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(2.0))),
                [],
                UnwindTarget::Propagate,
            );
            let right_value = builder.operation_results(right_value)[0];
            let right_jump = builder.terminate(
                OperationKind::Jump(JumpOp::new(BlockTarget::new(join, 0))),
                [],
                UnwindTarget::Propagate,
            );

            (
                left,
                right,
                join,
                left_value,
                right_value,
                left_jump,
                right_jump,
            )
        };

        let function = module.function_mut(function_id).unwrap();
        let mut editor = FunctionEditor::new(function);
        let parameters =
            editor.append_forwarded_block_parameters([join], |_, _, predecessor, _, _| {
                match predecessor {
                    block if block == left => left_value,
                    block if block == right => right_value,
                    _ => panic!("join has an unexpected predecessor"),
                }
            });

        let [(parameter_block, parameter)] = parameters.as_slice() else {
            panic!("one merge block must produce one parameter");
        };
        assert_eq!(*parameter_block, join);

        let block = editor.ir().block(join).unwrap();
        assert_eq!(block.parameters().len(), 1);
        assert_eq!(
            block.parameters()[0].source(),
            BlockParameterSource::Forwarded
        );
        assert_eq!(block.parameters()[0].value(), *parameter);

        assert_eq!(
            editor.ir().operation(left_jump).unwrap().operands(),
            [left_value],
        );
        assert_eq!(
            editor.ir().operation(right_jump).unwrap().operands(),
            [right_value],
        );
    }

    #[test]
    fn preserves_later_successor_arguments_when_inserting_an_earlier_one() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (left, terminator, left_argument, added_argument, right_argument) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let left = builder.create_block();
            let right = builder.create_block();
            let completion = builder.create_block();

            builder.append_block_parameter(left, BlockParameterSource::Forwarded);
            builder.append_block_parameter(right, BlockParameterSource::Forwarded);

            let condition = builder.append_operation(
                OperationKind::Constant(ConstantOp::new(ConstantValue::Boolean(true))),
                [],
                UnwindTarget::Propagate,
            );
            let condition = builder.operation_results(condition)[0];
            let left_argument = builder.append_operation(
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(1.0))),
                [],
                UnwindTarget::Propagate,
            );
            let left_argument = builder.operation_results(left_argument)[0];
            let added_argument = builder.append_operation(
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(2.0))),
                [],
                UnwindTarget::Propagate,
            );
            let added_argument = builder.operation_results(added_argument)[0];
            let right_argument = builder.append_operation(
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(3.0))),
                [],
                UnwindTarget::Propagate,
            );
            let right_argument = builder.operation_results(right_argument)[0];
            let terminator = builder.terminate(
                OperationKind::If(IfOp::new(
                    BlockTarget::new(left, 1),
                    BlockTarget::new(right, 1),
                    completion,
                )),
                [condition, left_argument, right_argument],
                UnwindTarget::Propagate,
            );

            (
                left,
                terminator,
                left_argument,
                added_argument,
                right_argument,
            )
        };

        let function = module.function_mut(function_id).unwrap();
        let mut editor = FunctionEditor::new(function);
        editor.append_forwarded_block_parameters([left], |_, _, _, _, _| added_argument);

        let terminator = editor.ir().operation(terminator).unwrap();
        assert_eq!(
            terminator.operands(),
            [
                terminator.operands()[0],
                left_argument,
                added_argument,
                right_argument,
            ],
        );

        let successors = terminator.successors();
        assert_eq!(
            successors[0].arguments(terminator.operands()),
            [left_argument, added_argument],
        );
        assert_eq!(
            successors[1].arguments(terminator.operands()),
            [right_argument],
        );
        assert_eq!(
            editor.ir().value(left_argument).unwrap().uses()[0].operand_index(),
            1,
        );
        assert_eq!(
            editor.ir().value(added_argument).unwrap().uses()[0].operand_index(),
            2,
        );
        assert_eq!(
            editor.ir().value(right_argument).unwrap().uses()[0].operand_index(),
            3,
        );
    }

    #[test]
    fn allocates_all_cyclic_parameters_before_requesting_arguments() {
        let mut module = ModuleIr::new();
        let function_id = module.entry_function();

        let (first, second, entry_jump, first_jump, second_jump, entry_value) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let first = builder.create_block();
            let second = builder.create_block();
            let entry_value = builder.append_operation(
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(0.0))),
                [],
                UnwindTarget::Propagate,
            );
            let entry_value = builder.operation_results(entry_value)[0];
            let entry_jump = builder.terminate(
                OperationKind::Jump(JumpOp::new(BlockTarget::new(first, 0))),
                [],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(first);
            let first_jump = builder.terminate(
                OperationKind::Jump(JumpOp::new(BlockTarget::new(second, 0))),
                [],
                UnwindTarget::Propagate,
            );

            builder.switch_to_block(second);
            let second_jump = builder.terminate(
                OperationKind::Jump(JumpOp::new(BlockTarget::new(first, 0))),
                [],
                UnwindTarget::Propagate,
            );

            (
                first,
                second,
                entry_jump,
                first_jump,
                second_jump,
                entry_value,
            )
        };

        let function = module.function_mut(function_id).unwrap();
        let mut editor = FunctionEditor::new(function);
        let parameters = editor.append_forwarded_block_parameters(
            [first, second],
            |parameters, target, predecessor, _, _| {
                let parameter = |block| {
                    parameters
                        .iter()
                        .find_map(|&(candidate, value)| (candidate == block).then_some(value))
                        .expect("every selected block must already have a parameter")
                };

                if target == first && predecessor != second {
                    entry_value
                } else if target == first {
                    parameter(second)
                } else {
                    parameter(first)
                }
            },
        );

        let first_parameter = parameters
            .iter()
            .find_map(|&(block, value)| (block == first).then_some(value))
            .unwrap();
        let second_parameter = parameters
            .iter()
            .find_map(|&(block, value)| (block == second).then_some(value))
            .unwrap();

        assert_eq!(
            editor.ir().operation(entry_jump).unwrap().operands(),
            [entry_value],
        );
        assert_eq!(
            editor.ir().operation(first_jump).unwrap().operands(),
            [first_parameter],
        );
        assert_eq!(
            editor.ir().operation(second_jump).unwrap().operands(),
            [second_parameter],
        );
    }
}
