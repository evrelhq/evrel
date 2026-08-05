//! Function-level JavaScript lowering state.

mod body;
mod completion;
mod control;
mod definition;
mod parameter;

use evrel_js_ir::{
    BindingId, BindingKind, BindingPattern, BlockId, BlockParameterSource, ExceptionHandlerId,
    FunctionBuilder, FunctionId, FunctionKind, FunctionMode, FunctionParameterKind,
    FunctionProperties, LabeledStatementData, LabeledStatementId, LocationId, OperationId,
    OperationKind, PrivateNameId, RegionId, TemplateSiteId, TextRange, ValueId,
};
use oxc_ast::ast::IdentifierReference;
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::Span;
use rustc_hash::FxHashMap;

use super::LoweringContext;

pub(crate) use body::{lower_function_body, lower_function_properties, lower_function_statements};
pub(crate) use definition::{
    lower_class_element_function, lower_object_method_function, lower_ordinary_function_definition,
};
pub(crate) use parameter::lower_function_parameters;

/// Lowers one JavaScript function into Evrel IR.
///
/// This owns frontend-specific lowering state. The wrapped `FunctionBuilder`
/// remains responsible for maintaining IR structural invariants.
pub(crate) struct FunctionLowerer<'ir, 'context, 'semantic> {
    builder: FunctionBuilder<'ir>,
    context: &'context mut LoweringContext<'semantic>,
    current_location: LocationId,
    control_frames: Vec<control::ControlFrame>,
}

impl<'ir, 'context, 'semantic> FunctionLowerer<'ir, 'context, 'semantic> {
    /// Creates a lowerer positioned at the function's entry block.
    pub(crate) fn new(
        builder: FunctionBuilder<'ir>,
        context: &'context mut LoweringContext<'semantic>,
        location: LocationId,
    ) -> Self {
        Self {
            builder,
            context,
            current_location: location,
            control_frames: Vec::new(),
        }
    }

    /// Builds a nested function and returns its ID and lowering result.
    pub(crate) fn build_nested_function<R>(
        &mut self,
        kind: FunctionKind,
        mode: FunctionMode,
        build: impl FnOnce(&mut FunctionLowerer<'_, '_, 'semantic>) -> R,
    ) -> (FunctionId, R) {
        self.build_nested_function_with_properties(kind, mode, FunctionProperties::default(), build)
    }

    /// Builds a nested function with immutable construction-time properties.
    pub(crate) fn build_nested_function_with_properties<R>(
        &mut self,
        kind: FunctionKind,
        mode: FunctionMode,
        properties: FunctionProperties,
        build: impl FnOnce(&mut FunctionLowerer<'_, '_, 'semantic>) -> R,
    ) -> (FunctionId, R) {
        let context = &mut *self.context;
        let location = self.current_location;

        self.builder
            .build_nested_function_with_properties(kind, mode, properties, |builder| {
                let mut lowerer = FunctionLowerer::new(builder, context, location);

                build(&mut lowerer)
            })
    }

    /// Lowers while one class's private names are lexically visible.
    pub(crate) fn with_private_name_scope<R>(
        &mut self,
        names: impl IntoIterator<Item = Box<str>>,
        lower: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let mut scope = FxHashMap::default();

        for name in names {
            if scope.contains_key(name.as_ref()) {
                continue;
            }

            let private_name = self.builder.create_private_name(name.clone());
            scope.insert(name, private_name);
        }

        self.context.push_private_name_scope(scope);
        let result = lower(self);
        self.context.pop_private_name_scope();

        result
    }

    /// Builds a one-result expression region in the current function context.
    pub(crate) fn build_expression_region(
        &mut self,
        build: impl FnOnce(&mut Self) -> Result<ValueId, crate::FrontendError>,
    ) -> Result<RegionId, crate::FrontendError> {
        let region = self.builder.begin_region(1);

        match build(self) {
            Ok(value) => {
                self.builder
                    .finish_region(region, self.current_location, [value]);
                Ok(region)
            }
            Err(error) => {
                self.builder.abandon_region(region);
                Err(error)
            }
        }
    }

    /// Resolves an identifier reference to its Evrel binding.
    ///
    /// Returns `None` only for unresolved runtime-global references.
    pub(crate) fn binding_for_reference(
        &self,
        identifier: &IdentifierReference<'_>,
    ) -> Option<BindingId> {
        self.context.binding_for_reference(identifier)
    }

    /// Returns the Evrel binding assigned to an Oxc symbol.
    pub(crate) fn binding_for_symbol(&self, symbol: SymbolId) -> BindingId {
        self.context.binding_for_symbol(symbol)
    }

    /// Resolves a lexically visible private name.
    pub(crate) fn private_name(&self, name: &str) -> PrivateNameId {
        self.context.private_name(name)
    }

    /// Creates a stable identity for one tagged-template syntax site.
    pub(crate) fn create_template_site(&mut self) -> TemplateSiteId {
        self.builder.create_template_site()
    }

    /// Returns the binding used by a default export without a source binding.
    pub(crate) fn default_export_binding(&self) -> Option<BindingId> {
        self.context.default_export_binding()
    }

    /// Returns Oxc's semantic model for the module being lowered.
    pub(crate) const fn scoping(&self) -> &Scoping {
        self.context.scoping()
    }

    /// Returns whether an Oxc symbol already has an Evrel binding.
    pub(crate) fn contains_binding(&self, symbol: SymbolId) -> bool {
        self.context.contains_binding(symbol)
    }

    /// Returns how the binding assigned to an Oxc symbol was declared.
    pub(crate) fn binding_kind_for_symbol(&self, symbol: SymbolId) -> BindingKind {
        let binding = self.binding_for_symbol(symbol);

        self.builder.binding_kind(binding)
    }

    /// Creates and registers a binding declared by this function.
    pub(crate) fn declare_binding(
        &mut self,
        symbol: SymbolId,
        name: impl Into<Box<str>>,
        kind: BindingKind,
    ) -> BindingId {
        let binding = self.builder.create_binding(name, kind);

        self.context.register_binding(symbol, binding);

        binding
    }

    /// Assigns the current function's internal name binding.
    pub(crate) fn set_self_binding(&mut self, binding: BindingId) {
        self.builder.set_self_binding(binding);
    }

    /// Appends a source-level parameter to this function's boundary.
    pub(crate) fn append_parameter(
        &mut self,
        kind: FunctionParameterKind,
        target: BindingPattern,
    ) -> ValueId {
        self.builder.append_parameter(kind, target)
    }

    /// Creates a block in the current function.
    ///
    /// The insertion point remains unchanged.
    pub(crate) fn create_block(&mut self) -> BlockId {
        self.builder.create_block()
    }

    /// Creates a catch handler nested inside the active unwind handler.
    pub(crate) fn create_catch_handler(
        &mut self,
        entry_block: BlockId,
    ) -> (ExceptionHandlerId, ValueId) {
        self.builder
            .create_catch_handler(self.active_exception_handler(), entry_block)
    }

    /// Creates a finally handler nested inside the active unwind handler.
    pub(crate) fn create_finally_handler(
        &mut self,
        entry_block: BlockId,
    ) -> (ExceptionHandlerId, ValueId) {
        let handler = self
            .builder
            .create_finally_handler(self.active_exception_handler(), entry_block);
        let exception = self.builder.append_block_parameter(
            entry_block,
            BlockParameterSource::Exception,
            evrel_js_ir::ValueType::JsValue,
        );

        (handler, exception)
    }
    /// Records source-level labeled-statement structure.
    pub(crate) fn create_labeled_statement(
        &mut self,
        data: LabeledStatementData,
    ) -> LabeledStatementId {
        self.builder.create_labeled_statement(data)
    }

    /// Appends an SSA parameter forwarded by predecessor operands.
    pub(crate) fn append_forwarded_block_parameter(&mut self, block: BlockId) -> ValueId {
        self.builder.append_block_parameter(
            block,
            BlockParameterSource::Forwarded,
            evrel_js_ir::ValueType::JsValue,
        )
    }

    /// Appends an SSA parameter created by a predecessor operation.
    pub(crate) fn append_produced_block_parameter(&mut self, block: BlockId) -> ValueId {
        self.builder.append_block_parameter(
            block,
            BlockParameterSource::Produced,
            evrel_js_ir::ValueType::JsValue,
        )
    }

    /// Appends the compiler-only completion parameter of a finalizer.
    pub(crate) fn append_completion_block_parameter(&mut self, block: BlockId) -> ValueId {
        self.builder.append_block_parameter(
            block,
            BlockParameterSource::Produced,
            evrel_js_ir::ValueType::Completion,
        )
    }

    /// Moves subsequent lowering to an existing block.
    pub(crate) fn switch_to_block(&mut self, block: BlockId) {
        self.builder.switch_to_block(block);
    }

    /// Returns whether the current insertion block already has a terminator.
    pub(crate) fn current_block_is_terminated(&self) -> bool {
        self.builder.current_block_is_terminated()
    }

    /// Returns whether the current function can read implicit `arguments`.
    pub(crate) fn has_arguments_environment(&self) -> bool {
        self.builder.has_arguments_environment()
    }

    /// Lowers within the source location of one syntax node.
    pub(crate) fn with_span<R>(&mut self, span: Span, lower: impl FnOnce(&mut Self) -> R) -> R {
        let location = self.location(span);
        let previous = std::mem::replace(&mut self.current_location, location);
        let result = lower(self);

        self.current_location = previous;

        result
    }

    /// Emits an operation at the current insertion block.
    pub(crate) fn emit(
        &mut self,
        kind: OperationKind,
        operands: impl IntoIterator<Item = ValueId>,
    ) -> Vec<ValueId> {
        let operands = operands.into_iter().collect::<Vec<_>>();

        let Some(handler) = self.local_exception_entry() else {
            let operation = self
                .builder
                .append_operation(self.current_location, kind, operands);

            return self.builder.operation_results(operation).to_vec();
        };

        if !self.builder.operation_effects(&kind).may_throw() {
            let operation = self
                .builder
                .append_operation(self.current_location, kind, operands);

            return self.builder.operation_results(operation).to_vec();
        }

        let normal = self.builder.create_block();
        let results = (0..kind.result_count())
            .map(|_| {
                self.builder.append_block_parameter(
                    normal,
                    BlockParameterSource::Produced,
                    evrel_js_ir::ValueType::JsValue,
                )
            })
            .collect::<Vec<_>>();

        self.builder.invoke(
            self.current_location,
            kind,
            evrel_js_ir::BlockTarget::new(normal, 0),
            evrel_js_ir::BlockTarget::new(handler, 0),
            operands,
        );
        self.builder.switch_to_block(normal);

        results
    }

    /// Emits an operation that must produce exactly one value.
    pub(crate) fn emit_value(
        &mut self,
        kind: OperationKind,
        operands: impl IntoIterator<Item = ValueId>,
    ) -> ValueId {
        let results = self.emit(kind, operands);

        let [result] = results.as_slice() else {
            panic!("value-producing operation must have exactly one result");
        };

        *result
    }

    /// Terminates the current insertion block.
    pub(crate) fn terminate(
        &mut self,
        kind: OperationKind,
        operands: impl IntoIterator<Item = ValueId>,
    ) -> OperationId {
        self.builder
            .terminate(self.current_location, kind, operands)
    }

    /// Terminates the current block by throwing a JavaScript value.
    pub(crate) fn terminate_throw(&mut self, value: ValueId) -> OperationId {
        let Some(handler) = self.local_exception_entry() else {
            return self.terminate(
                OperationKind::Throw(evrel_js_ir::ThrowOp::new(None)),
                [value],
            );
        };

        self.builder.terminate(
            self.current_location,
            OperationKind::Throw(evrel_js_ir::ThrowOp::new(Some(
                evrel_js_ir::BlockTarget::new(handler, 0),
            ))),
            [value],
        )
    }

    fn local_exception_entry(&self) -> Option<BlockId> {
        let handler = self.active_exception_handler()?;
        let handler = self
            .builder
            .exception_handler(handler)
            .expect("active exception handler must belong to the function");

        Some(handler.entry_block())
    }

    fn location(&mut self, span: Span) -> LocationId {
        self.builder.source_location(
            self.context.source_file(),
            TextRange::new(span.start, span.end),
        )
    }
}
