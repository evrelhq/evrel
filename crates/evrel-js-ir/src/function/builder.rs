//! Construction API for function IR.

use crate::{
    BindingId, BindingKind, BindingPattern, BlockId, BlockParameterSource, ExceptionHandlerData,
    ExceptionHandlerId, ExceptionHandlerKind, FunctionId, FunctionKind, FunctionMode,
    FunctionParameterKind, FunctionProperties, JsFunctionIr, JsModuleIr, LabeledStatementData,
    LabeledStatementId, LocationId, OperationId, OperationKind, OperationSuccessor, PrivateNameId,
    RegionId, RegionYieldOp, SourceFileId, SyntheticReason, TemplateSiteId, TextRange,
    UnwindTarget, ValueId,
};

/// Builds one function while tracking the current insertion block.
pub struct FunctionBuilder<'ir> {
    module: &'ir mut JsModuleIr,
    function: FunctionId,
    current_block: BlockId,
    current_region: RegionId,
    insertion_stack: Vec<(RegionId, BlockId)>,
}

impl<'ir> FunctionBuilder<'ir> {
    /// Creates a builder positioned at the function's entry block.
    pub(crate) fn new(module: &'ir mut JsModuleIr, function: FunctionId) -> Self {
        let function_ir = module
            .function(function)
            .expect("cannot build an unknown function");
        let current_region = function_ir.body_region();
        let current_block = function_ir.entry_block();

        Self {
            module,
            function,
            current_block,
            current_region,
            insertion_stack: Vec::new(),
        }
    }

    /// Returns the function being built.
    pub const fn function(&self) -> FunctionId {
        self.function
    }

    /// Returns the block that will receive new operations.
    pub const fn current_block(&self) -> BlockId {
        self.current_block
    }

    /// Returns whether this function inherits an implicit `arguments` binding.
    pub fn has_arguments_environment(&self) -> bool {
        let mut function = Some(self.function);

        while let Some(function_id) = function {
            let function_ir = self
                .module
                .function(function_id)
                .expect("function ancestry must remain live");

            match function_ir.kind() {
                FunctionKind::Ordinary
                | FunctionKind::ObjectMethod
                | FunctionKind::ClassConstructor
                | FunctionKind::ClassMethod => return true,
                FunctionKind::Arrow => function = function_ir.parent_function(),
                FunctionKind::Module
                | FunctionKind::ClassFieldInitializer
                | FunctionKind::ClassStaticBlock => return false,
            }
        }

        false
    }

    /// Returns whether the current insertion block already has a terminator.
    pub fn current_block_is_terminated(&self) -> bool {
        self.current_function()
            .block(self.current_block)
            .expect("current block must remain live")
            .terminator()
            .is_some()
    }

    /// Creates a binding declared by the function being built.
    pub fn create_binding(&mut self, name: impl Into<Box<str>>, kind: BindingKind) -> BindingId {
        self.module.create_binding(self.function, name, kind)
    }

    /// Creates a private name owned by the module.
    pub fn create_private_name(&mut self, name: impl Into<Box<str>>) -> PrivateNameId {
        self.module.create_private_name(name)
    }

    /// Creates a stable identity for one tagged-template syntax site.
    pub fn create_template_site(&mut self) -> TemplateSiteId {
        self.module.create_template_site()
    }

    /// Assigns the internal name binding of a named function expression.
    pub fn set_self_binding(&mut self, binding: BindingId) {
        let binding_data = self
            .module
            .binding(binding)
            .expect("self binding must belong to the module");

        assert_eq!(
            binding_data.declaring_function(),
            self.function,
            "self binding must be declared by the named function"
        );
        assert_eq!(
            binding_data.kind(),
            BindingKind::Function,
            "self binding must be a function binding"
        );

        self.current_function_mut().set_self_binding(binding);
    }

    /// Returns how a module-owned binding was declared.
    pub fn binding_kind(&self, binding: BindingId) -> BindingKind {
        self.module
            .binding(binding)
            .expect("binding must belong to the module")
            .kind()
    }

    /// Appends a source-level parameter to the function boundary.
    pub fn append_parameter(
        &mut self,
        kind: FunctionParameterKind,
        target: BindingPattern,
    ) -> ValueId {
        assert_ne!(
            self.current_function().kind(),
            FunctionKind::ClassStaticBlock,
            "class static blocks cannot have parameters"
        );

        for binding in target.binding_ids() {
            let binding = self
                .module
                .binding(binding)
                .expect("parameter pattern references an unknown binding");

            assert_eq!(
                binding.declaring_function(),
                self.function,
                "parameter binding must be declared by the function"
            );
        }

        for region in target.regions() {
            let body = self.current_function().body_region();
            let data = self
                .current_function()
                .region(region)
                .expect("parameter pattern references an unknown region");

            assert_eq!(
                data.parent(),
                Some(body),
                "parameter expression regions must be nested in the function body"
            );
            assert!(data.owner().is_none(), "parameter region is already owned");
        }

        self.current_function_mut().append_parameter(kind, target)
    }

    /// Builds a nested function and returns its ID and the closure's result.
    pub fn build_nested_function<R>(
        &mut self,
        kind: FunctionKind,
        mode: FunctionMode,
        build: impl FnOnce(FunctionBuilder<'_>) -> R,
    ) -> (FunctionId, R) {
        self.build_nested_function_with_properties(kind, mode, FunctionProperties::default(), build)
    }

    /// Builds a nested function with immutable construction-time properties.
    pub fn build_nested_function_with_properties<R>(
        &mut self,
        kind: FunctionKind,
        mode: FunctionMode,
        properties: FunctionProperties,
        build: impl FnOnce(FunctionBuilder<'_>) -> R,
    ) -> (FunctionId, R) {
        let function =
            self.module
                .create_function_with_properties(kind, mode, self.function, properties);
        let builder = FunctionBuilder::new(self.module, function);
        let result = build(builder);

        (function, result)
    }

    /// Begins an inline region and moves the insertion point to its entry block.
    ///
    /// Call [`Self::finish_region`] after building its body.
    pub fn begin_region(&mut self, result_count: usize) -> RegionId {
        let parent = self.current_region;
        let region = self
            .current_function_mut()
            .create_region(parent, result_count);
        let entry_block = self
            .current_function()
            .region(region)
            .expect("region was just created")
            .entry_block();

        self.insertion_stack
            .push((self.current_region, self.current_block));
        self.current_region = region;
        self.current_block = entry_block;

        region
    }

    /// Completes the active region and restores the enclosing insertion point.
    pub fn finish_region(
        &mut self,
        region: RegionId,
        location: LocationId,
        values: impl IntoIterator<Item = ValueId>,
        unwind_target: UnwindTarget,
    ) {
        assert_eq!(
            self.current_region, region,
            "regions must finish in nesting order"
        );
        let values = values.into_iter().collect::<Vec<_>>();
        let expected = self
            .current_function()
            .region(region)
            .expect("active region must remain live")
            .result_count();
        assert_eq!(
            values.len(),
            expected,
            "region result count must match its signature"
        );

        self.terminate(
            location,
            OperationKind::RegionYield(RegionYieldOp::new(values.len())),
            values,
            unwind_target,
        );
        self.restore_enclosing_insertion_point();
    }

    /// Restores the enclosing insertion point after failed region construction.
    ///
    /// The unattached region remains unreachable from executable operations.
    pub fn abandon_region(&mut self, region: RegionId) {
        assert_eq!(
            self.current_region, region,
            "regions must be abandoned in nesting order"
        );

        self.restore_enclosing_insertion_point();
    }

    /// Creates a block and appends it to the function layout.
    ///
    /// The current insertion block does not change.
    pub fn create_block(&mut self) -> BlockId {
        let region = self.current_region;

        self.current_function_mut().create_block(region)
    }

    /// Creates a catch handler and its runtime-provided exception parameter.
    pub fn create_catch_handler(
        &mut self,
        parent: Option<ExceptionHandlerId>,
        entry_block: BlockId,
    ) -> (ExceptionHandlerId, ValueId) {
        self.validate_block(entry_block);

        assert!(
            self.current_function()
                .block(entry_block)
                .expect("catch entry block was validated above")
                .parameters()
                .is_empty(),
            "catch entry block must not already have parameters"
        );

        let handler =
            self.create_exception_handler(ExceptionHandlerKind::Catch, parent, entry_block);
        let exception = self
            .current_function_mut()
            .append_block_parameter(entry_block, BlockParameterSource::Exception);

        (handler, exception)
    }

    /// Creates a finally handler over an existing entry block.
    pub fn create_finally_handler(
        &mut self,
        parent: Option<ExceptionHandlerId>,
        entry_block: BlockId,
    ) -> ExceptionHandlerId {
        self.create_exception_handler(ExceptionHandlerKind::Finally, parent, entry_block)
    }

    fn create_exception_handler(
        &mut self,
        kind: ExceptionHandlerKind,
        parent: Option<ExceptionHandlerId>,
        entry_block: BlockId,
    ) -> ExceptionHandlerId {
        self.validate_block(entry_block);

        if let Some(parent) = parent {
            assert!(
                self.current_function().exception_handler(parent).is_some(),
                "parent exception handler must belong to the current function"
            );
        }

        self.current_function_mut()
            .create_exception_handler(ExceptionHandlerData::new(kind, parent, entry_block))
    }

    /// Records a source-level labeled statement over existing CFG blocks.
    pub fn create_labeled_statement(&mut self, data: LabeledStatementData) -> LabeledStatementId {
        for block in data.referenced_blocks() {
            self.validate_block(block);
        }

        let body_region = self
            .current_function()
            .block_region(data.body_block())
            .expect("labeled body block was validated above");
        let completion_region = self
            .current_function()
            .block_region(data.completion_block())
            .expect("labeled completion block was validated above");

        assert_eq!(
            body_region, completion_region,
            "labeled statement blocks must belong to the same region"
        );

        self.current_function_mut().create_labeled_statement(data)
    }

    /// Appends an SSA parameter to a block.
    pub fn append_block_parameter(
        &mut self,
        block: BlockId,
        source: BlockParameterSource,
    ) -> ValueId {
        self.validate_block(block);

        self.current_function_mut()
            .append_block_parameter(block, source)
    }

    /// Moves the insertion point to an existing block.
    pub fn switch_to_block(&mut self, block: BlockId) {
        self.validate_block(block);

        self.current_block = block;
    }

    /// Appends an operation with its exceptional control-flow destination.
    pub fn append_operation(
        &mut self,
        location: LocationId,
        kind: OperationKind,
        operands: impl IntoIterator<Item = ValueId>,
        unwind_target: UnwindTarget,
    ) -> OperationId {
        assert!(
            !kind.is_terminator(),
            "use FunctionBuilder::terminate for terminator operations"
        );
        self.validate_references(&kind);
        self.validate_unwind_target(unwind_target);

        let block = self.current_block;

        self.current_function_mut()
            .append_operation(block, location, unwind_target, kind, operands)
    }

    /// Terminates the current block with its exceptional control-flow destination.
    pub fn terminate(
        &mut self,
        location: LocationId,
        kind: OperationKind,
        operands: impl IntoIterator<Item = ValueId>,
        unwind_target: UnwindTarget,
    ) -> OperationId {
        assert!(kind.is_terminator(), "expected a terminator operation");
        self.validate_references(&kind);
        self.validate_unwind_target(unwind_target);

        let block = self.current_block;

        self.current_function_mut()
            .set_terminator(block, location, unwind_target, kind, operands)
    }

    /// Returns the canonical location for a source range.
    pub fn source_location(&mut self, file: SourceFileId, range: TextRange) -> LocationId {
        self.module.source_location(file, range)
    }

    /// Returns a canonical location for compiler-created IR.
    pub fn synthetic_location(
        &mut self,
        reason: SyntheticReason,
        origins: impl IntoIterator<Item = LocationId>,
    ) -> LocationId {
        self.module.synthetic_location(reason, origins)
    }

    /// Returns the values produced by an operation.
    pub fn operation_results(&self, operation: OperationId) -> &[ValueId] {
        self.current_function()
            .operation(operation)
            .expect("operation must belong to the function")
            .results()
    }

    fn current_function(&self) -> &JsFunctionIr {
        self.module
            .function(self.function)
            .expect("function must remain live")
    }

    fn current_function_mut(&mut self) -> &mut JsFunctionIr {
        self.module
            .function_mut(self.function)
            .expect("function must remain live")
    }

    fn validate_references(&self, kind: &OperationKind) {
        kind.visit_referenced_bindings(|binding| {
            assert!(
                self.module.binding(binding).is_some(),
                "operation references an unknown module binding"
            );
        });

        kind.visit_referenced_functions(|function| {
            assert!(
                self.module.function(function).is_some(),
                "operation references an unknown module function"
            );
        });

        for region in kind.regions() {
            let data = self
                .current_function()
                .region(region)
                .expect("operation references an unknown region");

            assert_eq!(
                data.parent(),
                Some(self.current_region),
                "operation region must be nested in its operation's region"
            );
            assert!(data.owner().is_none(), "operation region is already owned");
        }

        for successor in kind.successors() {
            self.validate_successor(successor);
        }

        for block in kind.structural_blocks() {
            self.validate_block(block);
        }

        match kind {
            OperationKind::While(operation) => {
                assert!(
                    ![
                        operation.body_target().block(),
                        operation.exit_target().block()
                    ]
                    .contains(&self.current_block),
                    "conditional-loop operation, body, and exit must use distinct blocks"
                );
            }

            OperationKind::DoWhile(operation) => {
                assert!(
                    ![
                        operation.body_target().block(),
                        operation.exit_target().block()
                    ]
                    .contains(&self.current_block),
                    "conditional-loop operation, body, and exit must use distinct blocks"
                );
            }

            OperationKind::For(operation) => {
                assert_ne!(
                    operation.test_target().block(),
                    self.current_block,
                    "classical-for host and test must use distinct blocks"
                );
                assert!(
                    !operation.structural_blocks().contains(&self.current_block),
                    "classical-for host must be distinct from every phase block"
                );

                self.validate_per_iteration_bindings(operation.per_iteration_bindings());
            }

            OperationKind::ForIn(operation) => {
                self.validate_per_iteration_bindings(operation.per_iteration_bindings());
            }

            OperationKind::ForOf(operation) => {
                self.validate_per_iteration_bindings(operation.per_iteration_bindings());

                if operation.kind().is_async() {
                    let function = self.current_function();

                    assert!(
                        function.kind() == FunctionKind::Module || function.mode().is_async(),
                        "for-await-of is only valid in modules and async functions"
                    );
                }
            }

            OperationKind::Await(_) => {
                let function = self.current_function();

                assert!(
                    function.kind() == FunctionKind::Module || function.mode().is_async(),
                    "await is only valid in modules and async functions"
                );
            }

            OperationKind::Yield(_) => {
                assert!(
                    self.current_function().mode().is_generator(),
                    "yield is only valid in generator functions"
                );
            }

            OperationKind::RegionYield(operation) => {
                let region = self.current_region;

                assert_ne!(
                    region,
                    self.current_function().body_region(),
                    "region yield is not valid in the function body"
                );

                let expected = self
                    .current_function()
                    .region(region)
                    .expect("active region must remain live")
                    .result_count();

                assert_eq!(
                    operation.value_count(),
                    expected,
                    "region yield arity must match its region"
                );
            }

            OperationKind::Return(_) => {
                assert_eq!(
                    self.current_region,
                    self.current_function().body_region(),
                    "return is only valid in the function body"
                );
            }

            _ => {}
        }
    }

    fn validate_per_iteration_bindings(&self, bindings: &[BindingId]) {
        for binding in bindings {
            assert_eq!(
                self.module
                    .binding(*binding)
                    .expect("per-iteration binding must remain live")
                    .declaring_function(),
                self.function,
                "per-iteration binding must belong to the loop's function"
            );
        }
    }

    fn validate_successor(&self, successor: OperationSuccessor) {
        let target = successor.target();
        let block = self
            .current_function()
            .block(target.block())
            .expect("control flow references an unknown block");

        let parameters = block.parameters();
        let produced_count = successor.produced_argument_count();

        assert!(
            produced_count <= parameters.len(),
            "successor produces too many block arguments"
        );

        let (produced, forwarded) = parameters.split_at(produced_count);

        assert!(
            produced
                .iter()
                .all(|parameter| parameter.source() == BlockParameterSource::Produced),
            "produced block parameters must precede forwarded parameters"
        );

        assert!(
            forwarded
                .iter()
                .all(|parameter| parameter.source() == BlockParameterSource::Forwarded),
            "ordinary control flow cannot target exception or produced parameters"
        );

        assert_eq!(
            target.argument_count(),
            forwarded.len(),
            "forwarded argument count must match forwarded block parameters"
        );

        assert_eq!(
            self.current_function().block_region(target.block()),
            Some(self.current_region),
            "control flow cannot cross a region boundary"
        );
    }

    fn validate_block(&self, block: BlockId) {
        assert!(
            self.current_function().block(block).is_some(),
            "control flow references an unknown block"
        );
        assert_eq!(
            self.current_function().block_region(block),
            Some(self.current_region),
            "control flow cannot cross a region boundary"
        );
    }

    fn validate_unwind_target(&self, target: UnwindTarget) {
        if let UnwindTarget::Handler(handler) = target {
            assert!(
                self.current_function().exception_handler(handler).is_some(),
                "unwind handler must belong to the current function"
            );
        }
    }

    fn restore_enclosing_insertion_point(&mut self) {
        let (region, block) = self
            .insertion_stack
            .pop()
            .expect("region insertion stack must not be empty");

        self.current_region = region;
        self.current_block = block;
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ArrayLiteralElement, ArrayLiteralOp, AwaitOp, BinaryOp, BinaryOperator, BindingKind,
        BindingPattern, BlockParameterSource, BlockTarget, ConstantOp, ConstantValue,
        CreateFunctionOp, ExceptionHandlerKind, ForInOp, FunctionKind, FunctionMode,
        FunctionParameterKind, JsModuleIr, JumpOp, LoopKind, ModuleBuilder, OperationKind,
        RegionYieldOp, ReturnOp, ThrowOp, UnwindTarget, ValueDefinition, WhileOp, YieldKind,
        YieldOp,
    };

    #[test]
    #[should_panic(expected = "await is only valid in modules and async functions")]
    fn rejects_await_in_a_synchronous_function() {
        let mut module = JsModuleIr::new();
        let entry = module.entry_function();
        let mut module_builder = ModuleBuilder::new(&mut module);
        let function =
            module_builder.create_function(FunctionKind::Ordinary, FunctionMode::Normal, entry);
        let mut builder = module_builder.function_builder(function);
        let constant = builder.append_operation(
            crate::LocationId::UNKNOWN,
            OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
            [],
            crate::UnwindTarget::Propagate,
        );
        let value = builder.operation_results(constant)[0];

        builder.append_operation(
            crate::LocationId::UNKNOWN,
            OperationKind::Await(AwaitOp::new()),
            [value],
            crate::UnwindTarget::Propagate,
        );
    }

    #[test]
    #[should_panic(expected = "yield is only valid in generator functions")]
    fn rejects_yield_in_a_non_generator_function() {
        let mut module = JsModuleIr::new();
        let entry = module.entry_function();
        let mut module_builder = ModuleBuilder::new(&mut module);
        let function =
            module_builder.create_function(FunctionKind::Ordinary, FunctionMode::Normal, entry);
        let mut builder = module_builder.function_builder(function);
        let constant = builder.append_operation(
            crate::LocationId::UNKNOWN,
            OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
            [],
            crate::UnwindTarget::Propagate,
        );
        let value = builder.operation_results(constant)[0];

        builder.append_operation(
            crate::LocationId::UNKNOWN,
            OperationKind::Yield(YieldOp::new(YieldKind::Value)),
            [value],
            crate::UnwindTarget::Propagate,
        );
    }

    #[test]
    fn appends_a_source_level_function_parameter() {
        let mut module = JsModuleIr::new();
        let entry = module.entry_function();

        let (function, binding, value) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let function =
                module_builder.create_function(FunctionKind::Arrow, FunctionMode::Normal, entry);
            let binding = module_builder.create_binding(function, "value", BindingKind::Parameter);
            let value = module_builder.function_builder(function).append_parameter(
                FunctionParameterKind::Argument,
                BindingPattern::binding(binding),
            );

            (function, binding, value)
        };

        let function = module.function(function).unwrap();
        let [parameter] = function.parameters() else {
            panic!("function must have one parameter");
        };

        assert_eq!(parameter.kind(), FunctionParameterKind::Argument);
        assert_eq!(parameter.target().as_binding(), Some(binding));
        assert_eq!(parameter.value(), value);
        assert_eq!(
            function.value(value).unwrap().definition(),
            &ValueDefinition::FunctionParameter { parameter_index: 0 }
        );
    }

    #[test]
    fn creates_a_binding_declared_by_the_current_function() {
        let mut module = JsModuleIr::new();
        let entry = module.entry_function();

        let (function, binding) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let function =
                module_builder.create_function(FunctionKind::Arrow, FunctionMode::Normal, entry);
            let binding = module_builder
                .function_builder(function)
                .create_binding("value", BindingKind::Parameter);

            (function, binding)
        };

        let binding = module.binding(binding).unwrap();

        assert_eq!(binding.declaring_function(), function);
        assert_eq!(binding.name(), "value");
        assert_eq!(binding.kind(), BindingKind::Parameter);
    }

    #[test]
    fn records_a_named_function_expression_self_binding() {
        let mut module = JsModuleIr::new();
        let entry = module.entry_function();

        let (function, binding) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let function =
                module_builder.create_function(FunctionKind::Ordinary, FunctionMode::Normal, entry);
            let mut builder = module_builder.function_builder(function);
            let binding = builder.create_binding("recurse", BindingKind::Function);

            builder.set_self_binding(binding);

            (function, binding)
        };

        assert_eq!(
            module.function(function).unwrap().self_binding(),
            Some(binding)
        );
    }

    #[test]
    fn creates_blocks_without_changing_the_insertion_point() {
        let mut module = JsModuleIr::new();
        let function_id = module.entry_function();
        let entry = module.function(function_id).unwrap().entry_block();

        let created = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);

            assert_eq!(builder.current_block(), entry);

            let created = builder.create_block();

            assert_eq!(builder.current_block(), entry);

            builder.switch_to_block(created);

            assert_eq!(builder.current_block(), created);

            created
        };

        let function = module.function(function_id).unwrap();
        let block_order = function
            .blocks()
            .map(|(block, _)| block)
            .collect::<Vec<_>>();

        assert_eq!(block_order, vec![entry, created]);
    }

    #[test]
    fn derives_loop_structure_from_its_terminator() {
        let mut module = JsModuleIr::new();
        let function_id = module.entry_function();

        let (test, body, exit) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let test = builder.create_block();
            let body = builder.create_block();
            let exit = builder.create_block();
            let condition = builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Boolean(true))),
                [],
                UnwindTarget::Propagate,
            );
            let condition = builder.operation_results(condition)[0];

            builder.terminate(
                crate::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(test, 0))),
                [],
                UnwindTarget::Propagate,
            );
            builder.switch_to_block(test);
            builder.terminate(
                crate::LocationId::UNKNOWN,
                OperationKind::While(WhileOp::new(
                    test,
                    BlockTarget::new(body, 0),
                    BlockTarget::new(exit, 0),
                    Box::new(["outer".into()]),
                )),
                [condition],
                UnwindTarget::Propagate,
            );

            (test, body, exit)
        };

        let function = module.function(function_id).unwrap();
        let (_, loop_operation) = function.loop_operations().next().unwrap();

        assert_eq!(loop_operation.kind(), LoopKind::While);
        assert_eq!(loop_operation.test_block(), Some(test));
        assert_eq!(loop_operation.body_block(), body);
        assert_eq!(loop_operation.continue_block(), test);
        assert_eq!(loop_operation.exit_block(), exit);
        assert_eq!(loop_operation.label(), Some("outer"));
        assert_eq!(function.loop_operations().count(), 1);
    }

    #[test]
    #[should_panic(expected = "conditional loop test, body, and exit must use distinct blocks")]
    fn rejects_a_while_header_that_is_also_its_body() {
        let mut module = JsModuleIr::new();
        let function_id = module.entry_function();
        let mut module_builder = ModuleBuilder::new(&mut module);
        let mut builder = module_builder.function_builder(function_id);
        let condition = builder.append_operation(
            crate::LocationId::UNKNOWN,
            OperationKind::Constant(ConstantOp::new(ConstantValue::Boolean(true))),
            [],
            UnwindTarget::Propagate,
        );
        let condition = builder.operation_results(condition)[0];
        let exit = builder.create_block();
        builder.terminate(
            crate::LocationId::UNKNOWN,
            OperationKind::While(WhileOp::new(
                builder.current_block(),
                BlockTarget::new(builder.current_block(), 0),
                BlockTarget::new(exit, 0),
                Box::new([]),
            )),
            [condition],
            UnwindTarget::Propagate,
        );
    }

    #[test]
    fn records_nested_exception_handlers() {
        let mut module = JsModuleIr::new();
        let function_id = module.entry_function();

        let (outer, inner, outer_entry, exception) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);

            let outer_entry = builder.create_block();
            let (outer, exception) = builder.create_catch_handler(None, outer_entry);

            let inner_entry = builder.create_block();
            let inner = builder.create_finally_handler(Some(outer), inner_entry);

            (outer, inner, outer_entry, exception)
        };

        let function = module.function(function_id).unwrap();
        let [parameter] = function.block(outer_entry).unwrap().parameters() else {
            panic!("catch entry block must have one exception parameter");
        };

        assert_eq!(parameter.source(), BlockParameterSource::Exception);
        assert_eq!(parameter.value(), exception);
        assert_eq!(
            function.exception_handler(outer).unwrap().kind(),
            ExceptionHandlerKind::Catch
        );
        assert_eq!(
            function.exception_handler(inner).unwrap().parent(),
            Some(outer)
        );
        assert_eq!(function.exception_handlers().count(), 2);
    }

    #[test]
    fn assigns_an_unwind_handler_to_operations() {
        let mut module = JsModuleIr::new();
        let function_id = module.entry_function();

        let (constant, terminator, handler) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let catch_entry = builder.create_block();
            let (handler, _) = builder.create_catch_handler(None, catch_entry);

            let constant = builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
                [],
                UnwindTarget::Handler(handler),
            );
            let value = builder.operation_results(constant)[0];
            let terminator = builder.terminate(
                crate::LocationId::UNKNOWN,
                OperationKind::Throw(ThrowOp::new()),
                [value],
                UnwindTarget::Handler(handler),
            );

            (constant, terminator, handler)
        };

        let function = module.function(function_id).unwrap();

        assert_eq!(
            function.operation(constant).unwrap().unwind_target(),
            UnwindTarget::Handler(handler)
        );
        assert!(
            !function
                .operation(constant)
                .unwrap()
                .kind()
                .intrinsic_effects()
                .may_throw()
        );
        assert!(
            function
                .operation(terminator)
                .unwrap()
                .kind()
                .intrinsic_effects()
                .may_throw()
        );
    }

    #[test]
    #[should_panic(expected = "unwind handler must enter the operation's region or an ancestor")]
    fn rejects_an_unwind_handler_in_a_descendant_region() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();
        let mut module_builder = ModuleBuilder::new(&mut module);
        let mut builder = module_builder.function_builder(function);
        let region = builder.begin_region(0);
        let handler_entry = builder.current_block();
        let (handler, _) = builder.create_catch_handler(None, handler_entry);

        builder.abandon_region(region);
        builder.append_operation(
            crate::LocationId::UNKNOWN,
            OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
            [],
            UnwindTarget::Handler(handler),
        );
    }

    #[test]
    fn appends_a_block_parameter() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let (block, parameter) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let block = builder.create_block();
            let parameter = builder.append_block_parameter(block, BlockParameterSource::Forwarded);

            (block, parameter)
        };

        let function = module.function(function).unwrap();

        let [block_parameter] = function.block(block).unwrap().parameters() else {
            panic!("expected one block parameter");
        };

        assert_eq!(block_parameter.source(), BlockParameterSource::Forwarded);
        assert_eq!(block_parameter.value(), parameter);
        assert_eq!(
            function.value(parameter).unwrap().definition(),
            &ValueDefinition::BlockParameter {
                block,
                parameter_index: 0,
            }
        );
    }

    #[test]
    fn appends_an_exception_block_parameter() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let (block, parameter) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let block = builder.create_block();
            let parameter = builder.append_block_parameter(block, BlockParameterSource::Exception);

            (block, parameter)
        };

        let function = module.function(function).unwrap();
        let [block_parameter] = function.block(block).unwrap().parameters() else {
            panic!("expected one block parameter");
        };

        assert_eq!(block_parameter.source(), BlockParameterSource::Exception);
        assert_eq!(block_parameter.value(), parameter);
        assert_eq!(
            function.value(parameter).unwrap().definition(),
            &ValueDefinition::BlockParameter {
                block,
                parameter_index: 0,
            }
        );
    }

    #[test]
    fn accepts_a_for_in_body_with_one_produced_parameter() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();
        let (body, property_key) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let body = builder.create_block();
            let exit = builder.create_block();
            let property_key = builder.append_block_parameter(body, BlockParameterSource::Produced);
            let object = builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Undefined)),
                [],
                crate::UnwindTarget::Propagate,
            );
            let object = builder.operation_results(object)[0];

            builder.terminate(
                crate::LocationId::UNKNOWN,
                OperationKind::ForIn(ForInOp::new(
                    BlockTarget::new(body, 0),
                    BlockTarget::new(exit, 0),
                    Box::new([]),
                    Box::new([]),
                )),
                [object],
                crate::UnwindTarget::Propagate,
            );

            (body, property_key)
        };

        let function = module.function(function).unwrap();
        let [parameter] = function.block(body).unwrap().parameters() else {
            panic!("for-in body must receive one parameter");
        };

        assert_eq!(parameter.source(), BlockParameterSource::Produced);
        assert_eq!(parameter.value(), property_key);
    }

    #[test]
    #[should_panic(
        expected = "ordinary control flow cannot target exception or produced parameters"
    )]
    fn rejects_an_ordinary_target_to_an_exception_parameter() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();
        let mut module_builder = ModuleBuilder::new(&mut module);
        let mut builder = module_builder.function_builder(function);
        let target = builder.create_block();

        builder.append_block_parameter(target, BlockParameterSource::Exception);
        builder.terminate(
            crate::LocationId::UNKNOWN,
            OperationKind::Jump(JumpOp::new(BlockTarget::new(target, 0))),
            [],
            crate::UnwindTarget::Propagate,
        );
    }

    #[test]
    fn appends_an_operation_and_creates_its_result() {
        let mut module = JsModuleIr::new();
        let function_id = module.entry_function();
        let entry = module.function(function_id).unwrap().entry_block();

        let operation = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);

            builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(42.0))),
                [],
                crate::UnwindTarget::Propagate,
            )
        };

        let function = module.function(function_id).unwrap();
        let operation_data = function
            .operation(operation)
            .expect("appended operation must be live");
        let [result] = operation_data.results() else {
            panic!("constant operation must produce exactly one result");
        };

        assert_eq!(function.operation_count(), 1);
        assert_eq!(function.value_count(), 1);
        assert_eq!(function.block(entry).unwrap().operations(), &[operation]);
        assert!(operation_data.operands().is_empty());
        assert_eq!(
            function.value(*result).unwrap().definition(),
            &ValueDefinition::OperationResult {
                operation,
                result_index: 0,
            }
        );
    }

    #[test]
    fn builds_and_attaches_an_inline_expression_region() {
        let mut module = JsModuleIr::new();
        let function_id = module.entry_function();
        let entry = module.function(function_id).unwrap().entry_block();

        let (region, owner) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let region = builder.begin_region(1);
            let constant = builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(42.0))),
                [],
                crate::UnwindTarget::Propagate,
            );
            let value = builder.operation_results(constant)[0];
            builder.finish_region(
                region,
                crate::LocationId::UNKNOWN,
                [value],
                crate::UnwindTarget::Propagate,
            );

            let owner = builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::ArrayLiteral(ArrayLiteralOp::new([ArrayLiteralElement::Value {
                    expression: region,
                }])),
                [],
                crate::UnwindTarget::Propagate,
            );

            (region, owner)
        };

        let function = module.function(function_id).unwrap();
        let region_data = function.region(region).unwrap();
        let region_entry = region_data.entry_block();

        assert_eq!(region_data.parent(), Some(function.body_region()));
        assert_eq!(
            region_data.owner(),
            Some(crate::RegionOwner::Operation(owner))
        );
        assert_eq!(function.block_region(region_entry), Some(region));
        assert_eq!(function.block(entry).unwrap().operations(), &[owner]);
        assert!(function.block(region_entry).unwrap().terminator().is_some());
        assert_eq!(function.operation(owner).unwrap().regions(), [region]);
    }

    #[test]
    fn tracks_binary_operation_operands_and_uses() {
        let mut module = JsModuleIr::new();
        let function_id = module.entry_function();
        let entry = module.function(function_id).unwrap().entry_block();

        let (left_operation, left, right_operation, right, binary_operation) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);

            let left_operation = builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(20.0))),
                [],
                crate::UnwindTarget::Propagate,
            );
            let left = builder.operation_results(left_operation)[0];

            let right_operation = builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(22.0))),
                [],
                crate::UnwindTarget::Propagate,
            );
            let right = builder.operation_results(right_operation)[0];

            let binary_operation = builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::Binary(BinaryOp::new(BinaryOperator::Add)),
                [left, right],
                crate::UnwindTarget::Propagate,
            );

            (
                left_operation,
                left,
                right_operation,
                right,
                binary_operation,
            )
        };

        let function = module.function(function_id).unwrap();
        let binary_data = function
            .operation(binary_operation)
            .expect("binary operation must be live");
        let [binary_result] = binary_data.results() else {
            panic!("binary operation must produce exactly one result");
        };

        assert_eq!(function.operation_count(), 3);
        assert_eq!(function.value_count(), 3);
        assert_eq!(
            function.block(entry).unwrap().operations(),
            &[left_operation, right_operation, binary_operation]
        );
        assert_eq!(binary_data.operands(), &[left, right]);
        assert_eq!(
            function.value(*binary_result).unwrap().definition(),
            &ValueDefinition::OperationResult {
                operation: binary_operation,
                result_index: 0,
            }
        );

        let [left_use] = function.value(left).unwrap().uses() else {
            panic!("left value must have exactly one use");
        };
        assert_eq!(left_use.operation(), binary_operation);
        assert_eq!(left_use.operand_index(), 0);

        let [right_use] = function.value(right).unwrap().uses() else {
            panic!("right value must have exactly one use");
        };
        assert_eq!(right_use.operation(), binary_operation);
        assert_eq!(right_use.operand_index(), 1);
    }

    #[test]
    fn stores_a_terminator_separately_from_ordinary_operations() {
        let mut module = JsModuleIr::new();
        let function_id = module.entry_function();
        let entry = module.function(function_id).unwrap().entry_block();

        let (constant, returned_value, terminator) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function_id);
            let constant = builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::Constant(ConstantOp::new(ConstantValue::Number(42.0))),
                [],
                crate::UnwindTarget::Propagate,
            );
            let returned_value = builder.operation_results(constant)[0];
            let terminator = builder.terminate(
                crate::LocationId::UNKNOWN,
                OperationKind::Return(ReturnOp::new()),
                [returned_value],
                crate::UnwindTarget::Propagate,
            );

            (constant, returned_value, terminator)
        };

        let function = module.function(function_id).unwrap();
        let block = function.block(entry).unwrap();
        let terminator_data = function.operation(terminator).unwrap();

        assert_eq!(function.operation_count(), 2);
        assert_eq!(block.operations(), &[constant]);
        assert_eq!(block.terminator(), Some(terminator));
        assert_eq!(terminator_data.operands(), &[returned_value]);
        assert!(terminator_data.results().is_empty());
    }

    #[test]
    #[should_panic(expected = "region yield is not valid in the function body")]
    fn rejects_a_region_yield_from_the_function_body() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();
        let mut module_builder = ModuleBuilder::new(&mut module);
        let mut builder = module_builder.function_builder(function);

        builder.terminate(
            crate::LocationId::UNKNOWN,
            OperationKind::RegionYield(RegionYieldOp::new(0)),
            [],
            crate::UnwindTarget::Propagate,
        );
    }

    #[test]
    #[should_panic(expected = "return is only valid in the function body")]
    fn rejects_a_return_from_an_inline_region() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();
        let mut module_builder = ModuleBuilder::new(&mut module);
        let mut builder = module_builder.function_builder(function);

        builder.begin_region(1);
        let constant = builder.append_operation(
            crate::LocationId::UNKNOWN,
            OperationKind::Constant(ConstantOp::new(ConstantValue::Number(42.0))),
            [],
            crate::UnwindTarget::Propagate,
        );
        let value = builder.operation_results(constant)[0];

        builder.terminate(
            crate::LocationId::UNKNOWN,
            OperationKind::Return(ReturnOp::new()),
            [value],
            crate::UnwindTarget::Propagate,
        );
    }

    #[test]
    fn accepts_a_valid_block_target() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();

        let (entry, target, terminator) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(function);
            let entry = builder.current_block();
            let target = builder.create_block();
            let terminator = builder.terminate(
                crate::LocationId::UNKNOWN,
                OperationKind::Jump(JumpOp::new(BlockTarget::new(target, 0))),
                [],
                crate::UnwindTarget::Propagate,
            );

            (entry, target, terminator)
        };

        let function = module.function(function).unwrap();
        let operation = function.operation(terminator).unwrap();

        assert_eq!(
            function.block(entry).unwrap().terminator(),
            Some(terminator)
        );
        assert_eq!(
            operation.kind(),
            &OperationKind::Jump(JumpOp::new(BlockTarget::new(target, 0)))
        );
    }

    #[test]
    #[should_panic(expected = "forwarded argument count must match forwarded block parameters")]
    fn rejects_a_block_target_with_the_wrong_argument_count() {
        let mut module = JsModuleIr::new();
        let function = module.entry_function();
        let mut module_builder = ModuleBuilder::new(&mut module);
        let mut builder = module_builder.function_builder(function);
        let target = builder.create_block();

        builder.terminate(
            crate::LocationId::UNKNOWN,
            OperationKind::Jump(JumpOp::new(BlockTarget::new(target, 1))),
            [],
            crate::UnwindTarget::Propagate,
        );
    }

    #[test]
    fn builds_a_nested_function_and_resumes_the_parent() {
        let mut module = JsModuleIr::new();
        let parent = module.entry_function();

        let (nested, create_function) = {
            let mut module_builder = ModuleBuilder::new(&mut module);
            let mut builder = module_builder.function_builder(parent);
            let (nested, returned_value) = builder.build_nested_function(
                FunctionKind::Arrow,
                FunctionMode::Normal,
                |mut nested_builder| {
                    let constant = nested_builder.append_operation(
                        crate::LocationId::UNKNOWN,
                        OperationKind::Constant(ConstantOp::new(ConstantValue::Number(42.0))),
                        [],
                        crate::UnwindTarget::Propagate,
                    );
                    let value = nested_builder.operation_results(constant)[0];

                    nested_builder.terminate(
                        crate::LocationId::UNKNOWN,
                        OperationKind::Return(ReturnOp::new()),
                        [value],
                        crate::UnwindTarget::Propagate,
                    );

                    value
                },
            );

            assert_eq!(builder.function(), parent);

            let create_function = builder.append_operation(
                crate::LocationId::UNKNOWN,
                OperationKind::CreateFunction(CreateFunctionOp::new(nested)),
                [],
                crate::UnwindTarget::Propagate,
            );

            assert_eq!(returned_value.index(), 0);

            (nested, create_function)
        };

        let parent_ir = module.function(parent).unwrap();
        let nested = module.function(nested).unwrap();

        assert_eq!(parent_ir.operation_count(), 1);
        assert_eq!(
            parent_ir
                .operation(create_function)
                .unwrap()
                .results()
                .len(),
            1
        );
        assert_eq!(nested.kind(), FunctionKind::Arrow);
        assert_eq!(nested.parent_function(), Some(parent));
        assert_eq!(nested.operation_count(), 2);
    }
}
