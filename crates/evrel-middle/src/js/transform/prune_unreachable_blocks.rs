//! Removal of blocks that cannot execute.

use evrel_js_ir::{BlockId, FunctionEditor, JsFunctionIr};
use rustc_hash::FxHashSet;

use crate::js::work_queue::WorkQueue;

/// Removes blocks unreachable from executable or structurally live roots.
///
/// Returns the number of removed blocks.
pub fn prune_unreachable_blocks(function: &mut JsFunctionIr) -> usize {
    let blocks = plan_unreachable_blocks(function);
    let removed = blocks.len();

    if !blocks.is_empty() {
        FunctionEditor::new(function).remove_blocks(blocks);
    }

    removed
}

fn plan_unreachable_blocks(function: &JsFunctionIr) -> Vec<BlockId> {
    let mut retained = FxHashSet::default();
    let mut work = WorkQueue::new();

    retain(function.entry_block(), &mut retained, &mut work);

    // Default parameter expressions execute as part of function entry.
    for parameter in function.parameters() {
        for region in parameter.target().regions() {
            let entry = function
                .region(region)
                .expect("function parameter must reference a live region")
                .entry_block();

            retain(entry, &mut retained, &mut work);
        }
    }

    // Handler and label metadata are not removed by this pass. Their blocks
    // therefore remain structural roots.
    for (_, handler) in function.exception_handlers() {
        retain(handler.entry_block(), &mut retained, &mut work);
    }

    for (_, statement) in function.labeled_statements() {
        retain(statement.body_block(), &mut retained, &mut work);
        retain(statement.completion_block(), &mut retained, &mut work);
    }

    while let Some(block) = work.pop() {
        let block = function
            .block(block)
            .expect("retained block must remain live while planning");

        for operation in block.operations().iter().copied().chain(block.terminator()) {
            let data = function
                .operation(operation)
                .expect("retained block must reference a live operation");

            for successor in data.successors() {
                retain(successor.target().block(), &mut retained, &mut work);
            }

            for structural_block in data.structural_blocks() {
                retain(structural_block, &mut retained, &mut work);
            }

            for region in data.regions() {
                let entry = function
                    .region(region)
                    .expect("operation must own a live region")
                    .entry_block();

                retain(entry, &mut retained, &mut work);
            }
        }
    }

    function
        .regions()
        .flat_map(|(_, region)| region.blocks().iter().copied())
        .filter(|block| !retained.contains(block))
        .collect()
}

fn retain(block: BlockId, retained: &mut FxHashSet<BlockId>, work: &mut WorkQueue<BlockId>) {
    if retained.insert(block) {
        work.push(block);
    }
}

#[cfg(test)]
mod tests {
    use evrel_js_ir::{
        BlockTarget, ConstantOp, ConstantValue, IfOp, JsModuleIr, LoadThisOp, ModuleBuilder,
        OperationKind, ReturnOp,
    };

    use super::prune_unreachable_blocks;

    #[test]
    fn removes_a_detached_block() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let detached = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let detached = builder.create_block();
            let returned = append_number(&mut builder, 1.0);

            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [returned],
            );

            builder.switch_to_block(detached);
            let returned = append_number(&mut builder, 2.0);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [returned],
            );

            detached
        };

        assert_eq!(
            prune_unreachable_blocks(module.function_mut(function).unwrap()),
            1
        );
        assert!(module.function(function).unwrap().block(detached).is_none());
        assert_eq!(
            prune_unreachable_blocks(module.function_mut(function).unwrap()),
            0
        );
    }

    #[test]
    fn preserves_structurally_referenced_blocks() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let completion = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let then_block = builder.create_block();
            let else_block = builder.create_block();
            let completion = builder.create_block();
            let condition = builder.append_operation(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::LoadThis(LoadThisOp::new()),
                [],
            );
            let condition = builder.operation_results(condition)[0];

            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::If(IfOp::new(
                    BlockTarget::new(then_block, 0),
                    BlockTarget::new(else_block, 0),
                    completion,
                )),
                [condition],
            );

            for block in [then_block, else_block] {
                builder.switch_to_block(block);
                let returned = append_number(&mut builder, 1.0);
                builder.terminate(
                    evrel_js_ir::LocationId::UNKNOWN,
                    OperationKind::Return(ReturnOp::new()),
                    [returned],
                );
            }

            builder.switch_to_block(completion);
            let returned = append_number(&mut builder, 2.0);
            builder.terminate(
                evrel_js_ir::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [returned],
            );

            completion
        };

        assert_eq!(
            prune_unreachable_blocks(module.function_mut(function).unwrap()),
            0
        );
        assert!(
            module
                .function(function)
                .unwrap()
                .block(completion)
                .is_some(),
        );
    }

    fn append_number(
        builder: &mut evrel_js_ir::FunctionBuilder<'_>,
        value: f64,
    ) -> evrel_js_ir::ValueId {
        let operation = builder.append_operation(
            evrel_js_ir::LocationId::UNKNOWN,
            OperationKind::Constant(ConstantOp::new(ConstantValue::Number(value))),
            [],
        );

        builder.operation_results(operation)[0]
    }
}
