//! Structured control-flow recognition and planning.

mod iterator;
mod loops;
mod sequence;
mod switch;
mod r#try;
mod value_flow;

use iterator::*;
use loops::*;
use sequence::*;
use switch::*;
use r#try::*;

use super::*;

/// Source-structured constructs indexed by their first executed block.
///
/// A loop must be recognized before its entry block is traversed. In
/// particular, a `do...while` operation is stored in its later test block,
/// even though execution enters through its body block.
#[derive(Debug)]

struct JsStructureIndex<'function> {
    loops: HashMap<BlockId, LoopOperation<'function>>,
    labeled_statements: HashMap<BlockId, &'function evrel_js_ir::LabeledStatementData>,
    exception_entries: HashSet<BlockId>,
    invoke_normal_entries: HashSet<BlockId>,
    completion_entries: HashSet<BlockId>,
    native_completion_blocks: HashSet<BlockId>,
}

impl<'function> JsStructureIndex<'function> {
    fn collect(
        function_id: FunctionId,
        function: &'function JsFunctionIr,
    ) -> Result<Self, JsCodegenError> {
        let mut loops = HashMap::new();

        for (_, loop_operation) in function.loop_operations() {
            let entry = loop_operation.entry_block();

            if loops.insert(entry, loop_operation).is_some() {
                return Err(JsCodegenError::UnsupportedControlFlow {
                    function: function_id,
                    reason: concat!(file!(), ":", line!()),
                });
            }
        }

        let mut labeled_statements = HashMap::new();

        for (_, statement) in function.labeled_statements() {
            let entry = statement.body_block();

            if labeled_statements.insert(entry, statement).is_some() {
                return Err(JsCodegenError::UnsupportedControlFlow {
                    function: function_id,
                    reason: concat!(file!(), ":", line!()),
                });
            }
        }

        let exception_entries = function
            .exception_handlers()
            .map(|(_, handler)| handler.entry_block())
            .collect();
        let invoke_normal_entries = function
            .operations()
            .filter_map(|(_, operation)| match operation.kind() {
                OperationKind::Invoke(invoke) => Some(invoke.normal_target().block()),
                _ => None,
            })
            .collect();
        let completion_entries = function
            .operations()
            .filter_map(|(_, operation)| match operation.kind() {
                OperationKind::EnterFinally(operation) => Some(operation.target().block()),
                _ => None,
            })
            .collect();
        let native_completion_blocks = function
            .operations()
            .flat_map(|(_, operation)| match operation.kind() {
                OperationKind::Try(operation) => operation
                    .finally_exception_block()
                    .into_iter()
                    .collect::<Vec<_>>(),
                OperationKind::ResumeCompletion(operation) => operation
                    .cases()
                    .iter()
                    .filter(|case| case.kind() != evrel_js_ir::CompletionKind::Normal)
                    .map(|case| case.target().block())
                    .collect(),
                _ => Vec::new(),
            })
            .collect();

        Ok(Self {
            loops,
            labeled_statements,
            exception_entries,
            invoke_normal_entries,
            completion_entries,
            native_completion_blocks,
        })
    }

    fn loop_at(&self, block: BlockId) -> Option<LoopOperation<'function>> {
        self.loops.get(&block).copied()
    }

    fn labeled_statement_at(
        &self,
        block: BlockId,
    ) -> Option<&'function evrel_js_ir::LabeledStatementData> {
        self.labeled_statements.get(&block).copied()
    }

    fn is_exception_entry(&self, block: BlockId) -> bool {
        self.exception_entries.contains(&block)
    }

    fn is_invoke_normal_entry(&self, block: BlockId) -> bool {
        self.invoke_normal_entries.contains(&block)
    }

    fn is_completion_entry(&self, block: BlockId) -> bool {
        self.completion_entries.contains(&block)
    }

    fn is_omittable_native_completion_block(
        &self,
        function: &JsFunctionIr,
        block: BlockId,
    ) -> bool {
        if !self.native_completion_blocks.contains(&block) {
            return false;
        }

        let Some(block) = function.block(block) else {
            return false;
        };
        let Some(terminator) = block
            .terminator()
            .and_then(|terminator| function.operation(terminator))
        else {
            return false;
        };

        block.operations().is_empty()
            && matches!(
                terminator.kind(),
                OperationKind::EnterFinally(_)
                    | OperationKind::Jump(_)
                    | OperationKind::Return(_)
                    | OperationKind::Throw(_)
            )
    }
}

/// One enclosing JavaScript control structure that a CFG jump may exit or
/// continue.
#[derive(Debug, Clone, Copy)]
struct ActiveControl<'label> {
    structure_entry: BlockId,
    produced_block: Option<BlockId>,
    continue_target: Option<BlockId>,
    break_target: BlockId,
    label: Option<&'label str>,
    completion_flag: Option<JsLocalId>,
}

/// Dynamic control context inherited by a nested planning traversal.
#[derive(Clone, Copy)]
struct ControlPlanningScope<'controls, 'function> {
    active_controls: &'controls [ActiveControl<'function>],
    exception_target: Option<BlockId>,
}

impl<'controls, 'function> ControlPlanningScope<'controls, 'function> {
    const fn new(
        active_controls: &'controls [ActiveControl<'function>],
        exception_target: Option<BlockId>,
    ) -> Self {
        Self {
            active_controls,
            exception_target,
        }
    }

    fn with_controls<'nested>(
        self,
        active_controls: &'nested [ActiveControl<'function>],
    ) -> ControlPlanningScope<'nested, 'function> {
        ControlPlanningScope {
            active_controls,
            exception_target: self.exception_target,
        }
    }

    const fn with_exception_target(self, exception_target: Option<BlockId>) -> Self {
        Self {
            exception_target,
            ..self
        }
    }
}

struct ControlPlanningContext<'function> {
    function_id: FunctionId,
    function: &'function JsFunctionIr,
    structures: JsStructureIndex<'function>,
    values: &'function DenseMap<ValueId, JsValueRepresentation>,
}

fn structured_transfer(
    target: BlockId,
    active_controls: &[ActiveControl<'_>],
) -> Option<JsControlStep> {
    for control in active_controls.iter().rev() {
        if target == control.break_target {
            return Some(JsControlStep::Break {
                label: control.label.map(Into::into),
                completion_flag: control.completion_flag,
            });
        }

        if control.continue_target == Some(target) {
            return Some(JsControlStep::Continue {
                label: control.label.map(Into::into),
            });
        }
    }

    None
}

/// Validated structured control plan for one function body.
#[derive(Debug)]
pub(crate) struct JsControlPlan {
    body: JsControlSequence,
}

impl JsControlPlan {
    pub(crate) fn build(
        function_id: FunctionId,
        function: &JsFunctionIr,
        values: &DenseMap<ValueId, JsValueRepresentation>,
        locals: &mut JsLocalAllocator,
    ) -> Result<Self, JsCodegenError> {
        let context = ControlPlanningContext {
            function_id,
            function,
            structures: JsStructureIndex::collect(function_id, function)?,
            values,
        };

        let mut visited = HashSet::new();
        let body = plan_sequence(
            &context,
            locals,
            function.entry_block(),
            None,
            &mut visited,
            ControlPlanningScope::new(&[], None),
        )?;
        let body_region = function.region(function.body_region()).ok_or(
            JsCodegenError::UnsupportedControlFlow {
                function: function_id,
                reason: concat!(file!(), ":", line!()),
            },
        )?;

        if body_region.blocks().iter().any(|block| {
            !visited.contains(block)
                && !context
                    .structures
                    .is_omittable_native_completion_block(function, *block)
        }) {
            return Err(JsCodegenError::UnsupportedControlFlow {
                function: function_id,
                reason: concat!(file!(), ":", line!()),
            });
        }

        Ok(Self { body })
    }

    pub(crate) const fn body(&self) -> &JsControlSequence {
        &self.body
    }
}
