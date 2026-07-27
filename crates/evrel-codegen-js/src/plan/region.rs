use std::collections::HashSet;

use evrel_ir::{BlockId, BlockParameterSource, FunctionIr, OperationKind, RegionId, ValueId};

use crate::JsCodegenError;

use super::JsEdgeKey;

#[derive(Debug)]
pub(crate) struct JsExpressionRegionPlan {
    root: JsExpressionRegionStep,
}

impl JsExpressionRegionPlan {
    pub(crate) fn build(function: &FunctionIr, region: RegionId) -> Result<Self, JsCodegenError> {
        let data = function
            .region(region)
            .ok_or(JsCodegenError::UnknownRegion { region })?;
        if data.result_count() != 1 {
            return Err(JsCodegenError::UnsupportedExpressionRegion { region });
        }

        Ok(Self {
            root: plan_step(
                function,
                region,
                data.entry_block(),
                None,
                &mut HashSet::new(),
            )?,
        })
    }

    pub(crate) const fn root(&self) -> &JsExpressionRegionStep {
        &self.root
    }

    pub(crate) fn visit_edges(&self, visit: &mut impl FnMut(JsEdgeKey)) {
        self.root.visit_edges(visit);
    }
}

#[derive(Debug)]
pub(crate) enum JsExpressionRegionStep {
    /// The current branch has reached the shared continuation.
    Complete,

    /// Emit one block, then follow its planned terminator.
    Block {
        block: BlockId,
        continuation: JsExpressionRegionContinuation,
    },
}

impl JsExpressionRegionStep {
    fn visit_edges(&self, visit: &mut impl FnMut(JsEdgeKey)) {
        let Self::Block { continuation, .. } = self else {
            return;
        };
        match continuation {
            JsExpressionRegionContinuation::Yield(_) => {}
            JsExpressionRegionContinuation::Jump { edge, next } => {
                visit(*edge);
                next.visit_edges(visit);
            }
            JsExpressionRegionContinuation::Branch {
                then_edge,
                then_step,
                else_edge,
                else_step,
                next,
                ..
            } => {
                visit(*then_edge);
                then_step.visit_edges(visit);
                visit(*else_edge);
                else_step.visit_edges(visit);
                if let Some(next) = next {
                    next.visit_edges(visit);
                }
            }
        }
    }
}

#[derive(Debug)]
pub(crate) enum JsExpressionRegionContinuation {
    /// Return the region's single result expression.
    Yield(ValueId),

    /// Apply one SSA edge transfer and continue with the target block.
    Jump {
        edge: JsEdgeKey,
        next: Box<JsExpressionRegionStep>,
    },
    /// Emit a conditional expression, optionally followed by shared work.
    Branch {
        condition: ValueId,
        then_edge: JsEdgeKey,
        then_step: Box<JsExpressionRegionStep>,
        else_edge: JsEdgeKey,
        else_step: Box<JsExpressionRegionStep>,
        next: Option<Box<JsExpressionRegionStep>>,
    },
}

fn plan_step(
    function: &FunctionIr,
    region: RegionId,
    block: BlockId,
    stop: Option<BlockId>,
    active: &mut HashSet<BlockId>,
) -> Result<JsExpressionRegionStep, JsCodegenError> {
    if function.block_region(block) != Some(region) {
        return Err(JsCodegenError::UnsupportedExpressionRegion { region });
    }
    if Some(block) == stop {
        return Ok(JsExpressionRegionStep::Complete);
    }
    if !active.insert(block) {
        return Err(JsCodegenError::UnsupportedExpressionRegion { region });
    }

    let block_data = function
        .block(block)
        .ok_or(JsCodegenError::UnknownBlock { block })?;
    if block_data
        .parameters()
        .iter()
        .any(|parameter| parameter.source() != BlockParameterSource::Forwarded)
    {
        return Err(JsCodegenError::UnsupportedExpressionRegion { region });
    }
    let terminator_id = block_data
        .terminator()
        .ok_or(JsCodegenError::UnsupportedExpressionRegion { region })?;
    let terminator = function
        .operation(terminator_id)
        .ok_or(JsCodegenError::UnknownOperation {
            operation: terminator_id,
        })?;

    let continuation = match terminator.kind() {
        OperationKind::RegionYield(yield_operation)
            if yield_operation.value_count() == 1 && terminator.operands().len() == 1 =>
        {
            JsExpressionRegionContinuation::Yield(terminator.operands()[0])
        }
        OperationKind::Jump(jump) => JsExpressionRegionContinuation::Jump {
            edge: JsEdgeKey::new(terminator_id, 0),
            next: Box::new(plan_step(
                function,
                region,
                jump.target().block(),
                stop,
                active,
            )?),
        },
        OperationKind::If(branch) => {
            let condition =
                *terminator
                    .operands()
                    .first()
                    .ok_or(JsCodegenError::MalformedOperation {
                        operation: terminator_id,
                    })?;
            let completion = branch.completion_block();
            let shares_continuation = function
                .block(completion)
                .is_some_and(|block| block.parameters().len() > 1);
            let branch_stop = shares_continuation.then_some(completion).or(stop);
            let then_step = plan_step(
                function,
                region,
                branch.then_target().block(),
                branch_stop,
                active,
            )?;
            let else_step = plan_step(
                function,
                region,
                branch.else_target().block(),
                branch_stop,
                active,
            )?;
            let next = if shares_continuation && stop != Some(completion) {
                Some(Box::new(plan_step(
                    function, region, completion, stop, active,
                )?))
            } else {
                None
            };
            JsExpressionRegionContinuation::Branch {
                condition,
                then_edge: JsEdgeKey::new(terminator_id, 0),
                then_step: Box::new(then_step),
                else_edge: JsEdgeKey::new(terminator_id, 1),
                else_step: Box::new(else_step),
                next,
            }
        }
        _ => return Err(JsCodegenError::UnsupportedExpressionRegion { region }),
    };

    active.remove(&block);
    Ok(JsExpressionRegionStep::Block {
        block,
        continuation,
    })
}
