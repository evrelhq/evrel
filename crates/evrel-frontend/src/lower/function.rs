//! Function-level JavaScript lowering state.

mod body;
mod control;
mod definition;
mod parameter;

use evrel_ir::{
    BindingId, BindingKind, BindingPattern, BlockId, BlockParameterSource, ExceptionHandlerId,
    FunctionBuilder, FunctionId, FunctionKind, FunctionMode, FunctionParameterKind,
    FunctionProperties, LabeledStatementData, LabeledStatementId, OperationId, OperationKind,
    PrivateNameId, RegionId, TemplateSiteId, UnwindTarget, ValueId,
};
use oxc_ast::ast::IdentifierReference;
use oxc_semantic::{Scoping, SymbolId};
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
    unwind_target: UnwindTarget,
    controls: Vec<control::ControlContext>,
}

impl<'ir, 'context, 'semantic> FunctionLowerer<'ir, 'context, 'semantic> {
    /// Creates a lowerer positioned at the function's entry block.
    pub(crate) fn new(
        builder: FunctionBuilder<'ir>,
        context: &'context mut LoweringContext<'semantic>,
    ) -> Self {
        Self {
            builder,
            context,
            unwind_target: UnwindTarget::Propagate,
            controls: Vec::new(),
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

        self.builder
            .build_nested_function_with_properties(kind, mode, properties, |builder| {
                let mut lowerer = FunctionLowerer::new(builder, context);

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
                    .finish_region(region, [value], self.unwind_target);
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
            .create_catch_handler(self.unwind_target.handler(), entry_block)
    }

    /// Creates a finally handler nested inside the active unwind handler.
    pub(crate) fn create_finally_handler(&mut self, entry_block: BlockId) -> ExceptionHandlerId {
        self.builder
            .create_finally_handler(self.unwind_target.handler(), entry_block)
    }

    /// Lowers while routing exceptions to one handler.
    pub(crate) fn with_unwind_handler<R>(
        &mut self,
        handler: ExceptionHandlerId,
        lower: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let previous = std::mem::replace(&mut self.unwind_target, UnwindTarget::Handler(handler));
        let result = lower(self);

        self.unwind_target = previous;

        result
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
        self.builder
            .append_block_parameter(block, BlockParameterSource::Forwarded)
    }

    /// Appends an SSA parameter created by a predecessor operation.
    pub(crate) fn append_produced_block_parameter(&mut self, block: BlockId) -> ValueId {
        self.builder
            .append_block_parameter(block, BlockParameterSource::Produced)
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

    /// Emits an operation at the current insertion block.
    pub(crate) fn emit(
        &mut self,
        kind: OperationKind,
        operands: impl IntoIterator<Item = ValueId>,
    ) -> OperationId {
        self.builder
            .append_operation(kind, operands, self.unwind_target)
    }

    /// Returns the values produced by an emitted operation.
    pub(crate) fn operation_results(&self, operation: OperationId) -> &[ValueId] {
        self.builder.operation_results(operation)
    }

    /// Emits an operation that must produce exactly one value.
    pub(crate) fn emit_value(
        &mut self,
        kind: OperationKind,
        operands: impl IntoIterator<Item = ValueId>,
    ) -> ValueId {
        let operation = self.emit(kind, operands);

        let [result] = self.builder.operation_results(operation) else {
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
        self.builder.terminate(kind, operands, self.unwind_target)
    }
}
