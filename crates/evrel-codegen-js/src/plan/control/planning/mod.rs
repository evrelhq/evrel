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
    labeled_statements: HashMap<BlockId, &'function evrel_ir::LabeledStatementData>,
    exception_entries: HashSet<BlockId>,
}

impl<'function> JsStructureIndex<'function> {
    fn collect(
        function_id: FunctionId,
        function: &'function FunctionIr,
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

        Ok(Self {
            loops,
            labeled_statements,
            exception_entries,
        })
    }

    fn loop_at(&self, block: BlockId) -> Option<LoopOperation<'function>> {
        self.loops.get(&block).copied()
    }

    fn labeled_statement_at(
        &self,
        block: BlockId,
    ) -> Option<&'function evrel_ir::LabeledStatementData> {
        self.labeled_statements.get(&block).copied()
    }

    fn is_exception_entry(&self, block: BlockId) -> bool {
        self.exception_entries.contains(&block)
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

struct ControlPlanningContext<'function> {
    function_id: FunctionId,
    function: &'function FunctionIr,
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
        function: &FunctionIr,
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
            &[],
        )?;
        let body_region = function.region(function.body_region()).ok_or(
            JsCodegenError::UnsupportedControlFlow {
                function: function_id,
                reason: concat!(file!(), ":", line!()),
            },
        )?;

        if body_region
            .blocks()
            .iter()
            .any(|block| !visited.contains(block))
        {
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
