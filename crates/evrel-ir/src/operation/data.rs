//! Common operation representation.

use crate::{BindingId, BlockId, FunctionId, OperationSuccessor, RegionId, UnwindTarget, ValueId};

use super::{
    ArrayLiteralElement, ArrayLiteralOp, AwaitOp, BinaryOp, CallOp, ConstantOp, ConstructOp,
    CreateClassOp, CreateFunctionOp, DebuggerOp, DeleteOp, DestructureAssignmentOp,
    DestructureBindingOp, DoWhileOp, DynamicImportOp, ForInOp, ForOfOp, ForOp, HasPrivateNameOp,
    IfOp, InitializeBindingOp, IsNullishOp, JsxElementOp, JsxFragmentOp, JumpOp, LoadArgumentsOp,
    LoadBindingOp, LoadGlobalOp, LoadPropertyOp, LoadSuperPropertyOp, LoadThisOp, LoopOperation,
    MemoryEffects, MetaPropertyOp, ObjectLiteralOp, OperationEffects, RegExpLiteralOp,
    RegionYieldOp, ReturnOp, StoreBindingOp, StoreGlobalOp, StorePropertyOp, StoreSuperPropertyOp,
    SuperCallOp, SwitchOp, TaggedTemplateOp, TemplateLiteralOp, ThrowOp, TryOp, TypeofOp,
    TypeofTarget, UnaryOp, UpdateOp, WhileOp, YieldOp,
};

/// Data stored for an executable IR instruction.
#[derive(Debug, Clone, PartialEq)]
pub struct OperationData {
    block: BlockId,
    unwind_target: UnwindTarget,
    kind: OperationKind,
    operands: Vec<ValueId>,
    results: Vec<ValueId>,
}

impl OperationData {
    pub(crate) fn new(
        block: BlockId,
        unwind_target: UnwindTarget,
        kind: OperationKind,
        operands: Vec<ValueId>,
    ) -> Self {
        Self {
            block,
            unwind_target,
            kind,
            operands,
            results: Vec::new(),
        }
    }

    /// Returns the block containing this operation.
    pub const fn block(&self) -> BlockId {
        self.block
    }

    /// Returns where exceptions raised by this operation leave normal control flow.
    ///
    /// This records the operation's unwind context even when the current operation
    /// kind cannot throw intrinsically. Propagation is an explicit function exit,
    /// not an unknown or synthetic catch.
    pub const fn unwind_target(&self) -> UnwindTarget {
        self.unwind_target
    }

    /// Returns the operation-specific behavior.
    pub const fn kind(&self) -> &OperationKind {
        &self.kind
    }

    /// Returns the values consumed by this operation.
    pub fn operands(&self) -> &[ValueId] {
        &self.operands
    }

    /// Returns the values produced by this operation.
    pub fn results(&self) -> &[ValueId] {
        &self.results
    }

    /// Returns inline regions owned by this operation in semantic order.
    pub fn regions(&self) -> Vec<RegionId> {
        self.kind.regions()
    }

    /// Returns executable CFG successors in semantic order.
    pub fn successors(&self) -> Vec<OperationSuccessor> {
        self.kind.successors()
    }

    /// Returns structurally referenced non-successor blocks.
    pub fn structural_blocks(&self) -> Vec<BlockId> {
        self.kind.structural_blocks()
    }

    /// Returns this operation as a source-structured loop, when applicable.
    pub fn as_loop(&self) -> Option<LoopOperation<'_>> {
        LoopOperation::from_operation(self)
    }

    pub(crate) fn add_result(&mut self, value: ValueId) {
        self.results.push(value);
    }

    pub(crate) fn replace_operand(
        &mut self,
        operand_index: usize,
        replacement: ValueId,
    ) -> ValueId {
        let operand = self
            .operands
            .get_mut(operand_index)
            .expect("cannot replace an unknown operation operand");

        std::mem::replace(operand, replacement)
    }

    pub(crate) fn append_successor_argument(
        &mut self,
        successor_index: usize,
        argument: ValueId,
    ) -> usize {
        let operand_index = self
            .successors()
            .get(successor_index)
            .copied()
            .expect("operation has no such successor")
            .argument_operand_range()
            .end;

        self.operands.insert(operand_index, argument);

        self.kind
            .successor_target_mut(successor_index)
            .append_argument();

        debug_assert_eq!(self.operands.len(), self.kind.operand_count(),);

        operand_index
    }
}

/// The behavior performed by an IR operation.
#[derive(Debug, Clone, PartialEq)]
pub enum OperationKind {
    Constant(ConstantOp),
    RegExpLiteral(RegExpLiteralOp),
    TemplateLiteral(TemplateLiteralOp),
    TaggedTemplate(TaggedTemplateOp),
    ArrayLiteral(ArrayLiteralOp),
    ObjectLiteral(ObjectLiteralOp),
    JsxElement(JsxElementOp),
    JsxFragment(JsxFragmentOp),
    CreateFunction(CreateFunctionOp),
    CreateClass(CreateClassOp),
    LoadThis(LoadThisOp),
    LoadArguments(LoadArgumentsOp),
    MetaProperty(MetaPropertyOp),
    DynamicImport(DynamicImportOp),
    Debugger(DebuggerOp),
    InitializeBinding(InitializeBindingOp),
    DestructureBinding(DestructureBindingOp),
    DestructureAssignment(DestructureAssignmentOp),
    LoadBinding(LoadBindingOp),
    StoreBinding(StoreBindingOp),
    LoadGlobal(LoadGlobalOp),
    StoreGlobal(StoreGlobalOp),
    LoadProperty(LoadPropertyOp),
    LoadSuperProperty(LoadSuperPropertyOp),
    StoreProperty(StorePropertyOp),
    StoreSuperProperty(StoreSuperPropertyOp),
    HasPrivateName(HasPrivateNameOp),
    IsNullish(IsNullishOp),
    Typeof(TypeofOp),
    Delete(DeleteOp),
    Unary(UnaryOp),
    Update(UpdateOp),
    Binary(BinaryOp),
    Await(AwaitOp),
    Yield(YieldOp),
    Call(CallOp),
    SuperCall(SuperCallOp),
    Construct(ConstructOp),
    Jump(JumpOp),
    If(IfOp),
    Try(TryOp),
    While(WhileOp),
    DoWhile(DoWhileOp),
    For(ForOp),
    ForIn(ForInOp),
    ForOf(ForOfOp),
    Switch(SwitchOp),
    RegionYield(RegionYieldOp),
    Return(ReturnOp),
    Throw(ThrowOp),
}

impl OperationKind {
    /// Returns context-independent memory effects intrinsic to this operation.
    ///
    /// Memory effects from owned regions are calculated by
    /// [`FunctionIr::operation_memory_effects`](crate::FunctionIr::operation_memory_effects).
    pub fn intrinsic_memory_effects(&self) -> MemoryEffects {
        match self {
            Self::Constant(_)
            | Self::RegExpLiteral(_)
            | Self::CreateFunction(_)
            | Self::LoadThis(_)
            | Self::MetaProperty(_)
            | Self::IsNullish(_)
            | Self::Jump(_)
            | Self::If(_)
            | Self::While(_)
            | Self::DoWhile(_)
            | Self::For(_)
            | Self::RegionYield(_)
            | Self::Return(_)
            | Self::Throw(_) => MemoryEffects::NONE,

            Self::InitializeBinding(_) | Self::StoreBinding(_) => MemoryEffects::WRITE,
            Self::LoadArguments(_) | Self::LoadBinding(_) | Self::HasPrivateName(_) => {
                MemoryEffects::READ
            }

            Self::Typeof(operation) if matches!(operation.target(), TypeofTarget::Value) => {
                MemoryEffects::NONE
            }
            Self::Delete(operation) if matches!(operation.target(), super::DeleteTarget::Value) => {
                MemoryEffects::NONE
            }
            Self::Unary(operation)
                if matches!(
                    operation.operator(),
                    super::UnaryOperator::LogicalNot | super::UnaryOperator::Void
                ) =>
            {
                MemoryEffects::NONE
            }
            Self::Binary(operation)
                if matches!(
                    operation.operator(),
                    super::BinaryOperator::StrictEqual | super::BinaryOperator::StrictNotEqual
                ) =>
            {
                MemoryEffects::NONE
            }
            Self::TemplateLiteral(operation) if operation.substitutions().is_empty() => {
                MemoryEffects::NONE
            }
            Self::ArrayLiteral(operation)
                if operation
                    .elements()
                    .iter()
                    .all(|element| matches!(element, ArrayLiteralElement::Elision)) =>
            {
                MemoryEffects::NONE
            }
            Self::ObjectLiteral(operation) if operation.regions().is_empty() => MemoryEffects::NONE,

            // These operations may execute arbitrary JavaScript, access host
            // state, traverse user-controlled protocols, consult mutable
            // prototypes, or suspend across an external mutation.
            Self::TemplateLiteral(_)
            | Self::TaggedTemplate(_)
            | Self::ArrayLiteral(_)
            | Self::ObjectLiteral(_)
            | Self::JsxElement(_)
            | Self::JsxFragment(_)
            | Self::CreateClass(_)
            | Self::DynamicImport(_)
            | Self::Debugger(_)
            | Self::DestructureBinding(_)
            | Self::DestructureAssignment(_)
            | Self::LoadGlobal(_)
            | Self::StoreGlobal(_)
            | Self::LoadProperty(_)
            | Self::LoadSuperProperty(_)
            | Self::StoreProperty(_)
            | Self::StoreSuperProperty(_)
            | Self::Typeof(_)
            | Self::Delete(_)
            | Self::Unary(_)
            | Self::Update(_)
            | Self::Binary(_)
            | Self::Await(_)
            | Self::Yield(_)
            | Self::Call(_)
            | Self::SuperCall(_)
            | Self::Construct(_)
            | Self::Try(_)
            | Self::ForIn(_)
            | Self::ForOf(_)
            | Self::Switch(_) => MemoryEffects::UNKNOWN,
        }
    }

    /// Returns effects intrinsic to this operation.
    ///
    /// Effects from owned regions are calculated by
    /// [`FunctionIr::operation_effects`](crate::FunctionIr::operation_effects).
    pub fn intrinsic_effects(&self) -> OperationEffects {
        match self {
            Self::Constant(_)
            | Self::RegExpLiteral(_)
            | Self::CreateFunction(_)
            | Self::LoadArguments(_)
            | Self::MetaProperty(_)
            | Self::IsNullish(_)
            | Self::Jump(_)
            | Self::If(_)
            | Self::While(_)
            | Self::DoWhile(_)
            | Self::RegionYield(_)
            | Self::Return(_) => OperationEffects::NONE,

            Self::LoadThis(_)
            | Self::LoadGlobal(_)
            | Self::LoadProperty(_)
            | Self::LoadSuperProperty(_)
            | Self::HasPrivateName(_)
            | Self::Typeof(_)
            | Self::Update(_)
            | Self::Call(_)
            | Self::SuperCall(_)
            | Self::Construct(_)
            | Self::Throw(_) => OperationEffects::MAY_THROW,

            Self::StoreGlobal(_) | Self::StoreProperty(_) | Self::StoreSuperProperty(_) => {
                OperationEffects::MAY_THROW_AND_OBSERVABLE
            }

            Self::Await(_) | Self::Yield(_) => OperationEffects::MAY_THROW_OR_SUSPEND,

            Self::DynamicImport(_) | Self::Debugger(_) => OperationEffects::OBSERVABLE,

            Self::ArrayLiteral(operation) => operation.effects(),
            Self::ObjectLiteral(operation) => operation.effects(),
            Self::JsxElement(operation) => operation.effects(),
            Self::JsxFragment(operation) => operation.effects(),
            Self::TemplateLiteral(operation) => operation.effects(),
            Self::TaggedTemplate(operation) => operation.effects(),
            Self::InitializeBinding(operation) => operation.effects(),
            Self::CreateClass(operation) => operation.effects(),
            Self::DestructureBinding(operation) => operation.effects(),
            Self::DestructureAssignment(operation) => operation.effects(),
            Self::LoadBinding(operation) => operation.effects(),
            Self::StoreBinding(operation) => operation.effects(),
            Self::Delete(operation) => operation.effects(),
            Self::Unary(operation) => operation.effects(),
            Self::Binary(operation) => operation.effects(),
            Self::Try(operation) => operation.effects(),
            Self::For(operation) => operation.effects(),
            Self::ForIn(operation) => operation.effects(),
            Self::ForOf(operation) => operation.effects(),
            Self::Switch(operation) => operation.effects(),
        }
    }

    pub(crate) fn operand_count(&self) -> usize {
        match self {
            Self::Constant(operation) => operation.operand_count(),
            Self::RegExpLiteral(operation) => operation.operand_count(),
            Self::TemplateLiteral(operation) => operation.operand_count(),
            Self::TaggedTemplate(operation) => operation.operand_count(),
            Self::ArrayLiteral(operation) => operation.operand_count(),
            Self::ObjectLiteral(operation) => operation.operand_count(),
            Self::JsxElement(operation) => operation.operand_count(),
            Self::JsxFragment(operation) => operation.operand_count(),
            Self::CreateFunction(operation) => operation.operand_count(),
            Self::CreateClass(operation) => operation.operand_count(),
            Self::LoadThis(operation) => operation.operand_count(),
            Self::LoadArguments(operation) => operation.operand_count(),
            Self::MetaProperty(operation) => operation.operand_count(),
            Self::DynamicImport(operation) => operation.operand_count(),
            Self::Debugger(operation) => operation.operand_count(),
            Self::InitializeBinding(operation) => operation.operand_count(),
            Self::DestructureBinding(operation) => operation.operand_count(),
            Self::DestructureAssignment(operation) => operation.operand_count(),
            Self::LoadBinding(operation) => operation.operand_count(),
            Self::StoreBinding(operation) => operation.operand_count(),
            Self::LoadGlobal(operation) => operation.operand_count(),
            Self::StoreGlobal(operation) => operation.operand_count(),
            Self::LoadProperty(operation) => operation.operand_count(),
            Self::LoadSuperProperty(operation) => operation.operand_count(),
            Self::StoreProperty(operation) => operation.operand_count(),
            Self::StoreSuperProperty(operation) => operation.operand_count(),
            Self::HasPrivateName(operation) => operation.operand_count(),
            Self::IsNullish(operation) => operation.operand_count(),
            Self::Typeof(operation) => operation.operand_count(),
            Self::Delete(operation) => operation.operand_count(),
            Self::Unary(operation) => operation.operand_count(),
            Self::Update(operation) => operation.operand_count(),
            Self::Binary(operation) => operation.operand_count(),
            Self::Await(operation) => operation.operand_count(),
            Self::Yield(operation) => operation.operand_count(),
            Self::Call(operation) => operation.operand_count(),
            Self::SuperCall(operation) => operation.operand_count(),
            Self::Construct(operation) => operation.operand_count(),
            Self::Jump(operation) => operation.operand_count(),
            Self::If(operation) => operation.operand_count(),
            Self::Try(operation) => operation.operand_count(),
            Self::While(operation) => operation.operand_count(),
            Self::DoWhile(operation) => operation.operand_count(),
            Self::For(operation) => operation.operand_count(),
            Self::ForIn(operation) => operation.operand_count(),
            Self::ForOf(operation) => operation.operand_count(),
            Self::Switch(operation) => operation.operand_count(),
            Self::RegionYield(operation) => operation.operand_count(),
            Self::Return(operation) => operation.operand_count(),
            Self::Throw(operation) => operation.operand_count(),
        }
    }

    pub(crate) const fn result_count(&self) -> usize {
        match self {
            Self::Constant(operation) => operation.result_count(),
            Self::RegExpLiteral(operation) => operation.result_count(),
            Self::TemplateLiteral(operation) => operation.result_count(),
            Self::TaggedTemplate(operation) => operation.result_count(),
            Self::ArrayLiteral(operation) => operation.result_count(),
            Self::ObjectLiteral(operation) => operation.result_count(),
            Self::JsxElement(operation) => operation.result_count(),
            Self::JsxFragment(operation) => operation.result_count(),
            Self::CreateFunction(operation) => operation.result_count(),
            Self::CreateClass(operation) => operation.result_count(),
            Self::LoadThis(operation) => operation.result_count(),
            Self::LoadArguments(operation) => operation.result_count(),
            Self::MetaProperty(operation) => operation.result_count(),
            Self::DynamicImport(operation) => operation.result_count(),
            Self::Debugger(operation) => operation.result_count(),
            Self::InitializeBinding(operation) => operation.result_count(),
            Self::DestructureBinding(operation) => operation.result_count(),
            Self::DestructureAssignment(operation) => operation.result_count(),
            Self::LoadBinding(operation) => operation.result_count(),
            Self::StoreBinding(operation) => operation.result_count(),
            Self::LoadGlobal(operation) => operation.result_count(),
            Self::StoreGlobal(operation) => operation.result_count(),
            Self::LoadProperty(operation) => operation.result_count(),
            Self::LoadSuperProperty(operation) => operation.result_count(),
            Self::StoreProperty(operation) => operation.result_count(),
            Self::StoreSuperProperty(operation) => operation.result_count(),
            Self::HasPrivateName(operation) => operation.result_count(),
            Self::IsNullish(operation) => operation.result_count(),
            Self::Typeof(operation) => operation.result_count(),
            Self::Delete(operation) => operation.result_count(),
            Self::Unary(operation) => operation.result_count(),
            Self::Update(operation) => operation.result_count(),
            Self::Binary(operation) => operation.result_count(),
            Self::Await(operation) => operation.result_count(),
            Self::Yield(operation) => operation.result_count(),
            Self::Call(operation) => operation.result_count(),
            Self::SuperCall(operation) => operation.result_count(),
            Self::Construct(operation) => operation.result_count(),
            Self::Jump(operation) => operation.result_count(),
            Self::If(operation) => operation.result_count(),
            Self::Try(operation) => operation.result_count(),
            Self::While(operation) => operation.result_count(),
            Self::DoWhile(operation) => operation.result_count(),
            Self::For(operation) => operation.result_count(),
            Self::ForIn(operation) => operation.result_count(),
            Self::ForOf(operation) => operation.result_count(),
            Self::Switch(operation) => operation.result_count(),
            Self::RegionYield(operation) => operation.result_count(),
            Self::Return(operation) => operation.result_count(),
            Self::Throw(operation) => operation.result_count(),
        }
    }

    /// Visits every binding referenced by this operation.
    pub fn visit_referenced_bindings(&self, mut visit: impl FnMut(BindingId)) {
        match self {
            Self::CreateClass(operation) => {
                if let Some(binding) = operation.self_binding() {
                    visit(binding);
                }
            }
            Self::InitializeBinding(operation) => visit(operation.binding()),
            Self::DestructureBinding(operation) => {
                operation.pattern().visit_binding_ids(&mut visit);
            }
            Self::DestructureAssignment(operation) => {
                operation.pattern().visit_binding_ids(&mut visit);
            }
            Self::LoadBinding(operation) => visit(operation.binding()),
            Self::StoreBinding(operation) => visit(operation.binding()),
            Self::For(operation) => {
                for binding in operation.per_iteration_bindings() {
                    visit(*binding);
                }
            }
            Self::ForIn(operation) => {
                for binding in operation.per_iteration_bindings() {
                    visit(*binding);
                }
            }
            Self::ForOf(operation) => {
                for binding in operation.per_iteration_bindings() {
                    visit(*binding);
                }
            }
            _ => {}
        }
    }

    /// Visits every unresolved global identifier referenced by this operation.
    pub fn visit_referenced_global_names(&self, mut visit: impl FnMut(&str)) {
        match self {
            Self::LoadGlobal(operation) => visit(operation.name()),
            Self::StoreGlobal(operation) => visit(operation.name()),
            Self::Typeof(operation) => {
                if let TypeofTarget::Global(name) = operation.target() {
                    visit(name);
                }
            }
            _ => {}
        }
    }

    /// Returns inline regions owned by this operation in semantic order.
    pub fn regions(&self) -> Vec<RegionId> {
        match self {
            Self::ArrayLiteral(operation) => operation.regions(),
            Self::ObjectLiteral(operation) => operation.regions(),
            Self::JsxElement(operation) => operation.regions(),
            Self::JsxFragment(operation) => operation.regions(),
            Self::TemplateLiteral(operation) => operation.regions(),
            Self::TaggedTemplate(operation) => operation.regions(),
            Self::Call(operation) => operation.regions(),
            Self::SuperCall(operation) => operation.regions(),
            Self::Construct(operation) => operation.regions(),
            Self::Switch(operation) => operation.regions(),
            Self::CreateClass(operation) => operation.regions(),
            Self::DestructureBinding(operation) => operation.regions(),
            Self::DestructureAssignment(operation) => operation.regions(),
            _ => Vec::new(),
        }
    }

    /// Visits every statically referenced module-owned function body.
    pub fn visit_referenced_functions(&self, mut visit: impl FnMut(FunctionId)) {
        match self {
            Self::CreateFunction(operation) => visit(operation.function()),
            Self::ObjectLiteral(operation) => {
                for function in operation.referenced_functions() {
                    visit(function);
                }
            }
            Self::CreateClass(operation) => {
                for function in operation.referenced_functions() {
                    visit(function);
                }
            }
            _ => {}
        }
    }

    pub(crate) const fn is_terminator(&self) -> bool {
        matches!(
            self,
            Self::Jump(_)
                | Self::If(_)
                | Self::Try(_)
                | Self::While(_)
                | Self::DoWhile(_)
                | Self::For(_)
                | Self::ForIn(_)
                | Self::ForOf(_)
                | Self::Switch(_)
                | Self::RegionYield(_)
                | Self::Return(_)
                | Self::Throw(_)
        )
    }

    /// Returns executable CFG successors in semantic order.
    pub fn successors(&self) -> Vec<OperationSuccessor> {
        match self {
            Self::Jump(operation) => {
                vec![OperationSuccessor::new(operation.target(), 0)]
            }

            Self::If(operation) => {
                let then_target = operation.then_target();

                vec![
                    OperationSuccessor::new(then_target, 1),
                    OperationSuccessor::new(
                        operation.else_target(),
                        1 + then_target.argument_count(),
                    ),
                ]
            }

            Self::Try(operation) => operation.successors(),

            Self::While(operation) => operation.successors(),

            Self::DoWhile(operation) => operation.successors(),

            Self::For(operation) => operation.successors(),

            Self::ForIn(operation) => operation.successors(),

            Self::ForOf(operation) => operation.successors(),

            Self::Switch(operation) => operation.successors(),

            _ => Vec::new(),
        }
    }

    pub(crate) fn successor_target_mut(
        &mut self,
        successor_index: usize,
    ) -> &mut super::BlockTarget {
        match self {
            Self::Jump(operation) => match successor_index {
                0 => &mut operation.target,
                _ => panic!("jump has no successor {successor_index}"),
            },

            Self::If(operation) => match successor_index {
                0 => &mut operation.then_target,
                1 => &mut operation.else_target,
                _ => panic!("if has no successor {successor_index}"),
            },

            Self::Try(operation) => match successor_index {
                0 => &mut operation.try_target,
                _ => panic!("try has no successor {successor_index}"),
            },

            Self::While(operation) => operation.successor_target_mut(successor_index),

            Self::DoWhile(operation) => operation.successor_target_mut(successor_index),

            Self::For(operation) => operation.successor_target_mut(successor_index),

            Self::ForIn(operation) => operation.successor_target_mut(successor_index),

            Self::ForOf(operation) => operation.successor_target_mut(successor_index),

            Self::Switch(operation) => {
                if successor_index < operation.cases.len() {
                    return &mut operation.cases[successor_index].target;
                }

                if successor_index == operation.cases.len() {
                    return operation
                        .no_match_target
                        .as_mut()
                        .expect("switch has no no-match successor");
                }

                panic!("switch has no successor {successor_index}");
            }

            _ => panic!("operation kind has no successor {successor_index}",),
        }
    }

    /// Returns structurally referenced non-successor blocks.
    pub fn structural_blocks(&self) -> Vec<BlockId> {
        match self {
            Self::If(operation) => vec![operation.completion_block()],
            Self::Try(operation) => operation.structural_blocks(),
            Self::While(operation) => operation.structural_blocks(),
            Self::DoWhile(operation) => operation.structural_blocks(),
            Self::For(operation) => operation.structural_blocks(),
            Self::Switch(operation) => vec![operation.completion_block()],
            _ => Vec::new(),
        }
    }

    /// Returns whether operand zero selects between two truthiness successors.
    pub const fn is_conditional_branch(&self) -> bool {
        matches!(self, Self::If(_) | Self::While(_) | Self::DoWhile(_))
    }
}
