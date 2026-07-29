//! Function-level IR storage.

use std::collections::HashSet;

use crate::arena::Arena;
use crate::{
    BasicBlockData, BindingId, BindingPattern, BlockId, BlockParameter, BlockParameterSource,
    ExceptionHandlerData, ExceptionHandlerId, FunctionId, FunctionKind, FunctionMode,
    FunctionParameter, FunctionParameterKind, FunctionProperties, LabeledStatementData,
    LabeledStatementId, LoopOperation, MemoryEffects, OperationData, OperationEffects, OperationId,
    OperationKind, RegionData, RegionId, RegionOwner, UnwindTarget, ValueData, ValueDefinition,
    ValueId, ValueUse,
};

/// Owns the regions, blocks, operations, and values for one function.
#[derive(Clone)]
pub struct FunctionIr {
    kind: FunctionKind,
    mode: FunctionMode,
    properties: FunctionProperties,
    parent_function: Option<FunctionId>,
    self_binding: Option<BindingId>,
    parameters: Vec<FunctionParameter>,
    body: RegionId,
    regions: Arena<RegionId, RegionData>,
    blocks: Arena<BlockId, BasicBlockData>,
    exception_handlers: Arena<ExceptionHandlerId, ExceptionHandlerData>,
    labeled_statements: Arena<LabeledStatementId, LabeledStatementData>,
    operations: Arena<OperationId, OperationData>,
    values: Arena<ValueId, ValueData>,
}

impl FunctionIr {
    /// Creates an empty function with a root body region and one entry block.
    pub(crate) fn new(
        kind: FunctionKind,
        mode: FunctionMode,
        parent_function: Option<FunctionId>,
        properties: FunctionProperties,
    ) -> Self {
        assert!(
            kind != FunctionKind::Module || mode == FunctionMode::Normal,
            "module entry functions must use normal execution"
        );
        assert!(
            kind != FunctionKind::Arrow || !mode.is_generator(),
            "arrow functions cannot be generators"
        );
        assert!(
            kind != FunctionKind::ClassStaticBlock || mode == FunctionMode::Normal,
            "class static blocks must use normal execution"
        );

        let mut blocks = Arena::new();
        let mut regions = Arena::new();
        let body = regions.alloc_with_id(|region| {
            let entry_block = blocks.alloc(BasicBlockData::new(region));

            RegionData::function_body(entry_block)
        });

        Self {
            kind,
            mode,
            properties,
            parent_function,
            self_binding: None,
            parameters: Vec::new(),
            body,
            regions,
            blocks,
            exception_handlers: Arena::new(),
            labeled_statements: Arena::new(),
            operations: Arena::new(),
            values: Arena::new(),
        }
    }

    /// Returns the function's semantic form.
    pub const fn kind(&self) -> FunctionKind {
        self.kind
    }

    /// Returns the function's invocation and completion protocol.
    pub const fn mode(&self) -> FunctionMode {
        self.mode
    }

    /// Returns whether this function executes with strict-mode semantics.
    pub const fn is_strict(&self) -> bool {
        self.properties.is_strict()
    }

    /// Returns whether code generation must preserve an explicit
    /// `"use strict"` directive for this function body.
    pub const fn has_use_strict_directive(&self) -> bool {
        self.properties.has_use_strict_directive()
    }

    /// Returns the lexically enclosing function, if one exists.
    pub const fn parent_function(&self) -> Option<FunctionId> {
        self.parent_function
    }

    /// Returns the binding for a named function expression's own name.
    pub const fn self_binding(&self) -> Option<BindingId> {
        self.self_binding
    }

    /// Returns the source-level function parameters in declaration order.
    pub fn parameters(&self) -> &[FunctionParameter] {
        &self.parameters
    }

    /// Returns the function's root body region.
    pub const fn body_region(&self) -> RegionId {
        self.body
    }

    /// Returns the function body's entry block.
    pub fn entry_block(&self) -> BlockId {
        self.regions
            .get(self.body)
            .expect("function body region must remain live")
            .entry_block()
    }

    /// Returns the number of blocks in the function body.
    pub fn block_count(&self) -> usize {
        self.regions
            .get(self.body)
            .expect("function body region must remain live")
            .blocks()
            .len()
    }

    /// Returns the number of live operations.
    pub const fn operation_count(&self) -> usize {
        self.operations.len()
    }

    /// Returns the number of live values.
    pub const fn value_count(&self) -> usize {
        self.values.len()
    }

    /// Returns a block by ID.
    pub fn block(&self, id: BlockId) -> Option<&BasicBlockData> {
        self.blocks.get(id)
    }

    /// Returns an operation by ID.
    pub fn operation(&self, id: OperationId) -> Option<&OperationData> {
        self.operations.get(id)
    }

    /// Returns an operation's intrinsic effects and effects from its owned regions.
    pub fn operation_effects(&self, id: OperationId) -> Option<OperationEffects> {
        let operation = self.operation(id)?;
        let mut effects = operation.kind().intrinsic_effects();

        for region in operation.regions() {
            effects = effects.union(
                self.region_effects(region)
                    .expect("operation must reference a live region"),
            );
        }

        Some(effects)
    }

    /// Returns an operation's intrinsic memory effects and those of its owned regions.
    pub fn operation_memory_effects(&self, id: OperationId) -> Option<MemoryEffects> {
        let operation = self.operation(id)?;
        let mut effects = operation.kind().intrinsic_memory_effects();

        for region in operation.regions() {
            effects = effects.union(
                self.region_memory_effects(region)
                    .expect("operation must reference a live region"),
            );
        }

        Some(effects)
    }

    /// Iterates over live operations in allocation order.
    pub fn operations(&self) -> impl Iterator<Item = (OperationId, &OperationData)> + '_ {
        self.operations.iter()
    }

    /// Returns a value by ID.
    pub fn value(&self, id: ValueId) -> Option<&ValueData> {
        self.values.get(id)
    }

    /// Iterates over live values in allocation order.
    pub fn values(&self) -> impl Iterator<Item = (ValueId, &ValueData)> + '_ {
        self.values.iter()
    }

    /// Iterates over blocks in function-body layout order.
    pub fn blocks(&self) -> impl Iterator<Item = (BlockId, &BasicBlockData)> + '_ {
        self.region_blocks(self.body)
    }

    /// Returns a region by ID.
    pub fn region(&self, id: RegionId) -> Option<&RegionData> {
        self.regions.get(id)
    }

    /// Returns the combined effects of every operation in a region.
    pub fn region_effects(&self, id: RegionId) -> Option<OperationEffects> {
        let region = self.region(id)?;
        let mut effects = OperationEffects::NONE;

        for block in region.blocks() {
            let block = self
                .block(*block)
                .expect("region must reference a live block");

            for operation in block.operations() {
                effects = effects.union(
                    self.operation_effects(*operation)
                        .expect("block must reference a live operation"),
                );
            }

            if let Some(terminator) = block.terminator() {
                effects = effects.union(
                    self.operation_effects(terminator)
                        .expect("block must reference a live terminator"),
                );
            }
        }

        Some(effects)
    }

    /// Returns the combined memory effects of every operation in a region.
    pub fn region_memory_effects(&self, id: RegionId) -> Option<MemoryEffects> {
        let region = self.region(id)?;
        let mut effects = MemoryEffects::NONE;

        for block in region.blocks() {
            let block = self
                .block(*block)
                .expect("region must reference a live block");

            for operation in block.operations() {
                effects = effects.union(
                    self.operation_memory_effects(*operation)
                        .expect("block must reference a live operation"),
                );
            }

            if let Some(terminator) = block.terminator() {
                effects = effects.union(
                    self.operation_memory_effects(terminator)
                        .expect("block must reference a live terminator"),
                );
            }
        }

        Some(effects)
    }

    /// Iterates over regions in allocation order.
    pub fn regions(&self) -> impl Iterator<Item = (RegionId, &RegionData)> + '_ {
        self.regions.iter()
    }

    /// Returns the region containing a block.
    pub fn block_region(&self, block: BlockId) -> Option<RegionId> {
        self.blocks.get(block).map(BasicBlockData::region)
    }

    /// Iterates over one region's blocks in deterministic layout order.
    pub fn region_blocks(
        &self,
        region: RegionId,
    ) -> impl Iterator<Item = (BlockId, &BasicBlockData)> + '_ {
        self.regions
            .get(region)
            .expect("cannot iterate an unknown region")
            .blocks()
            .iter()
            .copied()
            .map(|id| {
                let block = self
                    .blocks
                    .get(id)
                    .expect("region layout must reference a live block");

                (id, block)
            })
    }

    /// Returns exception-handler metadata by ID.
    pub fn exception_handler(&self, id: ExceptionHandlerId) -> Option<&ExceptionHandlerData> {
        self.exception_handlers.get(id)
    }

    /// Iterates over exception handlers in allocation order.
    pub fn exception_handlers(
        &self,
    ) -> impl Iterator<Item = (ExceptionHandlerId, &ExceptionHandlerData)> + '_ {
        self.exception_handlers.iter()
    }

    pub(crate) fn create_exception_handler(
        &mut self,
        data: ExceptionHandlerData,
    ) -> ExceptionHandlerId {
        self.exception_handlers.alloc(data)
    }

    /// Returns labeled-statement metadata by ID.
    pub fn labeled_statement(&self, id: LabeledStatementId) -> Option<&LabeledStatementData> {
        self.labeled_statements.get(id)
    }

    /// Iterates over labeled statements in allocation order.
    pub fn labeled_statements(
        &self,
    ) -> impl Iterator<Item = (LabeledStatementId, &LabeledStatementData)> + '_ {
        self.labeled_statements.iter()
    }

    pub(crate) fn create_labeled_statement(
        &mut self,
        data: LabeledStatementData,
    ) -> LabeledStatementId {
        self.labeled_statements.alloc(data)
    }

    /// Iterates over source-structured loop operations in allocation order.
    pub fn loop_operations(&self) -> impl Iterator<Item = (OperationId, LoopOperation<'_>)> + '_ {
        self.operations.iter().filter_map(|(id, operation)| {
            operation
                .as_loop()
                .map(|loop_operation| (id, loop_operation))
        })
    }

    pub(crate) fn set_self_binding(&mut self, binding: BindingId) {
        assert!(
            self.self_binding.replace(binding).is_none(),
            "a function cannot have more than one self binding"
        );
    }

    pub(crate) fn append_parameter(
        &mut self,
        kind: FunctionParameterKind,
        target: BindingPattern,
    ) -> ValueId {
        let parameter_index =
            u32::try_from(self.parameters.len()).expect("function parameter count must fit in u32");
        let value = self
            .values
            .alloc(ValueData::new(ValueDefinition::FunctionParameter {
                parameter_index,
            }));

        for region in target.regions() {
            self.regions
                .get_mut(region)
                .expect("parameter pattern references an unknown region")
                .attach(RegionOwner::FunctionParameter { parameter_index });
        }

        self.parameters
            .push(FunctionParameter::new(kind, target, value));

        value
    }

    pub(crate) fn append_block_parameter(
        &mut self,
        block: BlockId,
        source: BlockParameterSource,
    ) -> ValueId {
        let parameter_index = {
            let block = self
                .blocks
                .get(block)
                .expect("cannot add a parameter to an unknown block");

            u32::try_from(block.parameters().len()).expect("block parameter count must fit in u32")
        };

        let value = self
            .values
            .alloc(ValueData::new(ValueDefinition::BlockParameter {
                block,
                parameter_index,
            }));

        self.blocks
            .get_mut(block)
            .expect("block was validated above")
            .add_parameter(BlockParameter::new(source, value));

        value
    }

    pub(crate) fn remove_operations(&mut self, operations: impl IntoIterator<Item = OperationId>) {
        let operations = operations.into_iter().collect::<Vec<_>>();
        let mut removal_set = HashSet::with_capacity(operations.len());

        for &operation in &operations {
            assert!(
                removal_set.insert(operation),
                "cannot remove the same operation twice",
            );
        }

        // Validate the complete batch before changing the function. Results
        // may be used by other operations in this batch, but not by operations
        // that will remain live.
        let removals = operations
            .iter()
            .copied()
            .map(|operation| {
                let data = self
                    .operations
                    .get(operation)
                    .expect("cannot remove an unknown operation");

                assert!(
                    !data.kind().is_terminator(),
                    "cannot remove a block terminator",
                );
                assert!(
                    data.regions().is_empty(),
                    "cannot remove an operation that owns regions",
                );

                for &result in data.results() {
                    let value = self
                        .values
                        .get(result)
                        .expect("operation result must remain live");

                    assert!(
                        value
                            .uses()
                            .iter()
                            .all(|use_site| removal_set.contains(&use_site.operation())),
                        "cannot remove an operation result with a live user",
                    );
                }

                (
                    operation,
                    data.block(),
                    data.operands().to_vec(),
                    data.results().to_vec(),
                )
            })
            .collect::<Vec<_>>();

        // Remove every operand use while all values are still live. This also
        // clears uses between operations in the removal set.
        for (operation, _, operands, _) in &removals {
            for (operand_index, &operand) in operands.iter().enumerate() {
                let operand_index =
                    u32::try_from(operand_index).expect("operand index must fit in u32");

                self.values
                    .get_mut(operand)
                    .expect("operation operand must remain live")
                    .remove_use(ValueUse::new(*operation, operand_index));
            }
        }

        for (operation, block, _, _) in &removals {
            self.blocks
                .get_mut(*block)
                .expect("operation block must remain live")
                .remove_operation(*operation);
        }

        for (_, _, _, results) in &removals {
            for &result in results {
                let value = self
                    .values
                    .remove(result)
                    .expect("operation result must remain live");

                debug_assert!(
                    value.uses().is_empty(),
                    "removed operation result must have no remaining uses",
                );
            }
        }

        for (operation, _, _, _) in removals {
            self.operations
                .remove(operation)
                .expect("validated operation must remain live");
        }
    }

    pub(crate) fn replace_operand(
        &mut self,
        operation: OperationId,
        operand_index: usize,
        replacement: ValueId,
    ) -> bool {
        assert!(
            self.values.get(replacement).is_some(),
            "replacement value must belong to the function",
        );

        let previous = *self
            .operations
            .get(operation)
            .expect("cannot edit an unknown operation")
            .operands()
            .get(operand_index)
            .expect("cannot replace an unknown operation operand");

        if previous == replacement {
            return false;
        }

        let operand_index = u32::try_from(operand_index).expect("operand index must fit in u32");
        let use_site = ValueUse::new(operation, operand_index);

        self.values
            .get_mut(previous)
            .expect("existing operand must remain live")
            .remove_use(use_site);

        self.values
            .get_mut(replacement)
            .expect("replacement was validated above")
            .add_use(use_site);

        let replaced = self
            .operations
            .get_mut(operation)
            .expect("operation was validated above")
            .replace_operand(operand_index as usize, replacement);

        debug_assert_eq!(replaced, previous);

        true
    }

    fn append_successor_argument(
        &mut self,
        operation: OperationId,
        successor_index: usize,
        argument: ValueId,
    ) {
        assert!(
            self.values.get(argument).is_some(),
            "successor argument must belong to the function",
        );

        let (operand_index, shifted_operands) = {
            let data = self
                .operations
                .get(operation)
                .expect("cannot edit an unknown operation");

            let operand_index = data
                .successors()
                .get(successor_index)
                .copied()
                .expect("operation has no such successor")
                .argument_operand_range()
                .end;

            (operand_index, data.operands()[operand_index..].to_vec())
        };

        for (offset, value) in shifted_operands.iter().copied().enumerate() {
            let shifted_index =
                u32::try_from(operand_index + offset).expect("operand index must fit in u32");

            self.values
                .get_mut(value)
                .expect("shifted operand must remain live")
                .remove_use(ValueUse::new(operation, shifted_index));
        }

        let inserted_index = self
            .operations
            .get_mut(operation)
            .expect("operation was validated above")
            .append_successor_argument(successor_index, argument);

        debug_assert_eq!(inserted_index, operand_index);

        self.values
            .get_mut(argument)
            .expect("argument was validated above")
            .add_use(ValueUse::new(
                operation,
                u32::try_from(operand_index).expect("operand index must fit in u32"),
            ));

        for (offset, value) in shifted_operands.into_iter().enumerate() {
            let shifted_index =
                u32::try_from(operand_index + offset + 1).expect("operand index must fit in u32");

            self.values
                .get_mut(value)
                .expect("shifted operand must remain live")
                .add_use(ValueUse::new(operation, shifted_index));
        }
    }

    pub(crate) fn append_forwarded_block_parameters(
        &mut self,
        blocks: impl IntoIterator<Item = BlockId>,
        mut argument_for_edge: impl FnMut(
            &[(BlockId, ValueId)],
            BlockId,
            BlockId,
            OperationId,
            usize,
        ) -> ValueId,
    ) -> Vec<(BlockId, ValueId)> {
        let blocks = blocks.into_iter().collect::<Vec<_>>();

        for (index, block) in blocks.iter().copied().enumerate() {
            assert!(
                !blocks[..index].contains(&block),
                "cannot append the same block parameter twice",
            );

            assert!(
                self.blocks.get(block).is_some(),
                "cannot add a parameter to an unknown block",
            );
        }

        let incoming_edges = blocks
            .iter()
            .copied()
            .map(|target| {
                let edges = self
                    .operations
                    .iter()
                    .flat_map(|(operation, data)| {
                        data.successors().into_iter().enumerate().filter_map(
                            move |(successor_index, successor)| {
                                (successor.target().block() == target).then_some((
                                    data.block(),
                                    operation,
                                    successor_index,
                                ))
                            },
                        )
                    })
                    .collect::<Vec<_>>();

                assert!(
                    !edges.is_empty(),
                    "a forwarded block parameter requires an incoming edge",
                );

                (target, edges)
            })
            .collect::<Vec<_>>();

        // Allocate all parameters before asking for incoming arguments so cyclic
        // merge blocks may refer to one another.
        let parameters = blocks
            .iter()
            .copied()
            .map(|block| {
                let parameter = self.append_block_parameter(block, BlockParameterSource::Forwarded);

                (block, parameter)
            })
            .collect::<Vec<_>>();

        let mut arguments = Vec::new();

        for ((target, edges), (_, parameter)) in incoming_edges.iter().zip(&parameters) {
            for &(predecessor, terminator, successor_index) in edges {
                let argument = argument_for_edge(
                    &parameters,
                    *target,
                    predecessor,
                    terminator,
                    successor_index,
                );

                assert!(
                    self.values.get(argument).is_some(),
                    "incoming argument must belong to the function",
                );

                arguments.push((terminator, successor_index, argument));
            }

            debug_assert_eq!(
                self.blocks
                    .get(*target)
                    .expect("parameter block must remain live")
                    .parameters()
                    .last()
                    .expect("a parameter was just appended")
                    .value(),
                *parameter,
            );
        }

        // Do not mutate any edge until every requested argument is valid.
        for (terminator, successor_index, argument) in arguments {
            self.append_successor_argument(terminator, successor_index, argument);
        }

        parameters
    }

    pub(crate) fn create_region(&mut self, parent: RegionId, result_count: usize) -> RegionId {
        assert!(
            self.regions.get(parent).is_some(),
            "parent region must belong to the function"
        );

        let blocks = &mut self.blocks;

        self.regions.alloc_with_id(|region| {
            let entry_block = blocks.alloc(BasicBlockData::new(region));

            RegionData::inline(parent, entry_block, result_count)
        })
    }

    pub(crate) fn create_block(&mut self, region: RegionId) -> BlockId {
        assert!(
            self.regions.get(region).is_some(),
            "cannot create a block in an unknown region"
        );

        let block = self.blocks.alloc(BasicBlockData::new(region));

        self.regions
            .get_mut(region)
            .expect("region was validated above")
            .append_block(block);

        block
    }

    pub(crate) fn append_operation(
        &mut self,
        block: BlockId,
        unwind_target: UnwindTarget,
        kind: OperationKind,
        operands: impl IntoIterator<Item = ValueId>,
    ) -> OperationId {
        assert!(
            !kind.is_terminator(),
            "cannot append a terminator as an ordinary operation"
        );

        let block_data = self
            .blocks
            .get(block)
            .expect("cannot append an operation to an unknown block");

        assert!(
            block_data.terminator().is_none(),
            "cannot append an operation after a block terminator"
        );

        let operation = self.create_operation(block, unwind_target, kind, operands);

        self.blocks
            .get_mut(block)
            .expect("block was validated above")
            .append_operation(operation);

        operation
    }

    pub(crate) fn set_terminator(
        &mut self,
        block: BlockId,
        unwind_target: UnwindTarget,
        kind: OperationKind,
        operands: impl IntoIterator<Item = ValueId>,
    ) -> OperationId {
        assert!(kind.is_terminator(), "expected a terminator operation");

        let block_data = self
            .blocks
            .get(block)
            .expect("cannot terminate an unknown block");

        assert!(
            block_data.terminator().is_none(),
            "a block cannot have more than one terminator"
        );

        let operation = self.create_operation(block, unwind_target, kind, operands);

        self.blocks
            .get_mut(block)
            .expect("block was validated above")
            .set_terminator(operation);

        operation
    }

    fn create_operation(
        &mut self,
        block: BlockId,
        unwind_target: UnwindTarget,
        kind: OperationKind,
        operands: impl IntoIterator<Item = ValueId>,
    ) -> OperationId {
        let operands = operands.into_iter().collect::<Vec<_>>();

        assert_eq!(
            operands.len(),
            kind.operand_count(),
            "operand count does not match operation kind"
        );

        for operand in &operands {
            assert!(
                self.values.get(*operand).is_some(),
                "operation references an unknown value"
            );
        }

        if let UnwindTarget::Handler(handler) = unwind_target {
            let handler = self
                .exception_handlers
                .get(handler)
                .expect("unwind handler must belong to the function");
            let source_region = self
                .block_region(block)
                .expect("operation block must belong to a live region");
            let handler_region = self
                .block_region(handler.entry_block())
                .expect("handler entry block must belong to a live region");
            let mut region = Some(source_region);
            let mut enters_same_or_ancestor_region = false;

            while let Some(candidate) = region {
                if candidate == handler_region {
                    enters_same_or_ancestor_region = true;
                    break;
                }

                region = self
                    .regions
                    .get(candidate)
                    .expect("operation region must remain live")
                    .parent();
            }

            assert!(
                enters_same_or_ancestor_region,
                "unwind handler must enter the operation's region or an ancestor"
            );
        }

        let regions = kind.regions();
        for region in &regions {
            assert!(
                self.regions.get(*region).is_some(),
                "operation references an unknown region"
            );
        }

        let result_count = kind.result_count();
        let operation = self.operations.alloc(OperationData::new(
            block,
            unwind_target,
            kind,
            operands.clone(),
        ));

        for region in regions {
            self.regions
                .get_mut(region)
                .expect("region was validated above")
                .attach(RegionOwner::Operation(operation));
        }

        for (operand_index, operand) in operands.into_iter().enumerate() {
            let operand_index = u32::try_from(operand_index)
                .expect("an operation cannot have more than u32::MAX operands");

            self.values
                .get_mut(operand)
                .expect("operand was validated above")
                .add_use(ValueUse::new(operation, operand_index));
        }

        for result_index in 0..result_count {
            let result_index = u32::try_from(result_index)
                .expect("an operation cannot have more than u32::MAX results");

            let result = self
                .values
                .alloc(ValueData::new(ValueDefinition::OperationResult {
                    operation,
                    result_index,
                }));

            self.operations
                .get_mut(operation)
                .expect("operation was just allocated")
                .add_result(result);
        }

        operation
    }
}

#[cfg(test)]
mod tests {
    use super::FunctionIr;
    use crate::{
        ArrayLiteralElement, ArrayLiteralOp, ConstantOp, ConstantValue, DebuggerOp, FunctionId,
        FunctionKind, FunctionMode, FunctionProperties, MemoryEffects, OperationEffects,
        OperationKind, RegionOwner, RegionYieldOp, UnwindTarget,
    };

    #[test]
    fn creates_a_function_with_one_entry_block() {
        let parent = FunctionId::from_index(0);
        let function = FunctionIr::new(
            FunctionKind::Ordinary,
            FunctionMode::Normal,
            Some(parent),
            FunctionProperties::default(),
        );
        let blocks = function.blocks().collect::<Vec<_>>();

        assert_eq!(function.block_count(), 1);
        assert_eq!(function.kind(), FunctionKind::Ordinary);
        assert_eq!(function.mode(), FunctionMode::Normal);
        assert_eq!(function.parent_function(), Some(parent));
        assert_eq!(function.operation_count(), 0);
        assert_eq!(function.value_count(), 0);
        assert_eq!(function.regions().count(), 1);
        assert_eq!(
            function.block_region(blocks[0].0),
            Some(function.body_region())
        );
        assert_eq!(
            function.region(function.body_region()).unwrap().owner(),
            Some(RegionOwner::FunctionBody)
        );
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, function.entry_block());
    }

    #[test]
    fn includes_owned_region_effects_in_operation_effects() {
        let mut function = FunctionIr::new(
            FunctionKind::Module,
            FunctionMode::Normal,
            None,
            FunctionProperties::default(),
        );
        let expression = function.create_region(function.body_region(), 1);
        let expression_block = function.region(expression).unwrap().entry_block();

        function.append_operation(
            expression_block,
            UnwindTarget::Propagate,
            OperationKind::Debugger(DebuggerOp::new()),
            [],
        );

        let value_operation = function.append_operation(
            expression_block,
            UnwindTarget::Propagate,
            OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
            [],
        );
        let value = function.operation(value_operation).unwrap().results()[0];

        function.set_terminator(
            expression_block,
            UnwindTarget::Propagate,
            OperationKind::RegionYield(RegionYieldOp::new(1)),
            [value],
        );

        let array = function.append_operation(
            function.entry_block(),
            UnwindTarget::Propagate,
            OperationKind::ArrayLiteral(ArrayLiteralOp::new([ArrayLiteralElement::Value {
                expression,
            }])),
            [],
        );

        assert_eq!(
            function.operation_effects(array),
            Some(OperationEffects::OBSERVABLE)
        );
        assert_eq!(
            function.region_memory_effects(expression),
            Some(MemoryEffects::UNKNOWN)
        );
        assert_eq!(
            function.operation_memory_effects(value_operation),
            Some(MemoryEffects::NONE)
        );
        assert_eq!(
            function.operation_memory_effects(array),
            Some(MemoryEffects::UNKNOWN)
        );
    }
}
