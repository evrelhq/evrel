//! Structured control-flow plans.

mod planning;

pub(crate) use planning::JsControlPlan;

use std::collections::{HashMap, HashSet};

use evrel_js_ir::{
    BlockId, BlockParameterSource, DoWhileOp, ForInOp, ForOfKind, ForOfOp, ForOp, FunctionId,
    JsFunctionIr, LoopOperation, OperationId, OperationKind, RegionId, ValueId, WhileOp,
};

use crate::JsCodegenError;

use super::{DenseMap, JsLocalAllocator, JsLocalId, JsValueRepresentation};

/// A structured sequence that emission can follow without rediscovering CFG
/// shape.
#[derive(Debug)]
pub(crate) struct JsControlSequence {
    steps: Vec<JsControlStep>,
}

impl JsControlSequence {
    pub(crate) fn steps(&self) -> &[JsControlStep] {
        &self.steps
    }

    fn prepend_edge(&mut self, edge: JsEdgeKey) {
        self.steps.insert(0, JsControlStep::Edge(edge));
    }

    pub(crate) fn visit_edges(&self, visit: &mut impl FnMut(JsEdgeKey)) {
        for step in &self.steps {
            match step {
                JsControlStep::Edge(edge) => visit(*edge),
                JsControlStep::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    then_branch.visit_edges(visit);
                    else_branch.visit_edges(visit);
                }
                JsControlStep::While {
                    body,
                    body_edge,
                    exit_edge,
                    ..
                } => {
                    body.visit_edges(visit);
                    visit(*body_edge);
                    visit(*exit_edge);
                }
                JsControlStep::WhileFlow { test, body, .. } => {
                    body.visit_edges(visit);
                    for &edge in test.value_flow().edges() {
                        visit(edge);
                    }
                    visit(test.body_edge());
                    visit(test.exit_edge());
                }
                JsControlStep::DoWhile { body, test, .. } => {
                    body.visit_edges(visit);
                    for &edge in test.value_flow().edges() {
                        visit(edge);
                    }
                    visit(test.body_edge());
                    visit(test.exit_edge());
                }
                JsControlStep::Labeled { body, .. } => body.visit_edges(visit),
                JsControlStep::For(plan) => {
                    plan.initializer.visit_edges(visit);
                    visit(plan.enter_test_edge);
                    plan.body.visit_edges(visit);
                    plan.update.visit_edges(visit);

                    match &plan.test {
                        JsForTestPlan::Always { body_edge, .. } => visit(*body_edge),
                        JsForTestPlan::Conditional {
                            body_edge,
                            exit_edge,
                            ..
                        } => {
                            visit(*body_edge);
                            visit(*exit_edge);
                        }
                        JsForTestPlan::Flow {
                            value_flow,
                            body_edge,
                            exit_edge,
                        } => {
                            for &edge in value_flow.edges() {
                                visit(edge);
                            }
                            visit(*body_edge);
                            visit(*exit_edge);
                        }
                    }
                }
                JsControlStep::Iterator(plan) => {
                    plan.body.visit_edges(visit);
                    visit(plan.natural_exit_edge);
                }
                JsControlStep::Switch(plan) => {
                    for case in &plan.cases {
                        visit(case.entry_edge);
                        case.body.visit_edges(visit);
                    }

                    if let Some(edge) = plan.no_match_edge {
                        visit(edge);
                    }
                }
                JsControlStep::Try(plan) => {
                    plan.try_body.visit_edges(visit);

                    if let Some(catch) = &plan.catch {
                        catch.body.visit_edges(visit);
                    }

                    if let Some(finally) = &plan.finally {
                        finally.visit_edges(visit);
                    }
                }
                JsControlStep::Block(_)
                | JsControlStep::Break { .. }
                | JsControlStep::Continue { .. } => {}
            }
        }
    }
}

/// Identifies one exact successor occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct JsEdgeKey {
    terminator: OperationId,
    successor_index: usize,
}

impl JsEdgeKey {
    pub(crate) const fn new(terminator: OperationId, successor_index: usize) -> Self {
        Self {
            terminator,
            successor_index,
        }
    }

    pub(crate) const fn terminator(self) -> OperationId {
        self.terminator
    }

    pub(crate) const fn successor_index(self) -> usize {
        self.successor_index
    }
}

/// One validated structured JavaScript action.
#[derive(Debug)]
pub(crate) enum JsControlStep {
    /// Emits the ordinary operations in one IR block.
    Block(BlockId),

    /// Executes the SSA transfer for one exact CFG edge.
    Edge(JsEdgeKey),

    /// Emits a structured JavaScript conditional.
    If {
        condition: ValueId,
        then_branch: JsControlSequence,
        else_branch: JsControlSequence,
    },

    /// Emits a pre-test loop while preserving the complete test block.
    While {
        labels: Box<[Box<str>]>,
        test_block: BlockId,
        condition: ValueId,
        body_edge: JsEdgeKey,
        body: JsControlSequence,
        exit_edge: JsEdgeKey,
    },

    /// Emits a pre-test loop whose test spans an acyclic CFG.
    WhileFlow {
        labels: Box<[Box<str>]>,
        test: JsFlowTestPlan,
        body: JsControlSequence,
    },

    /// Emits a post-test loop while preserving the body and test blocks.
    DoWhile {
        labels: Box<[Box<str>]>,
        body: JsControlSequence,
        test: JsFlowTestPlan,
    },

    /// Transfers to the exit of an enclosing control structure.
    Break {
        label: Option<Box<str>>,
        completion_flag: Option<JsLocalId>,
    },

    /// Transfers to the continuation point of an enclosing loop.
    Continue { label: Option<Box<str>> },

    /// Emits a non-loop labeled statement.
    Labeled {
        labels: Box<[Box<str>]>,
        body: JsControlSequence,
    },

    /// Emits a classical loop with explicit initializer, test, update, and body.
    For(JsForPlan),

    /// Emits a native property-enumeration or iterator loop.
    Iterator(JsIteratorPlan),

    /// Emits a source-ordered JavaScript switch.
    Switch(JsSwitchPlan),

    /// Emits a native JavaScript try statement.
    Try(JsTryPlan),
}

/// A validated body-region subgraph that produces one condition value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JsValueFlowPlan {
    root: JsValueFlowStep,
    result_block: BlockId,
    result: ValueId,
    edges: Box<[JsEdgeKey]>,
}

/// A multi-block loop test with its two terminal transfers.
///
/// Unlike [`JsForTestPlan`], this type cannot represent an unconditional or
/// single-block test and is therefore the only valid test for flow-shaped
/// `while` and `do...while` plans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JsFlowTestPlan {
    value_flow: JsValueFlowPlan,
    body_edge: JsEdgeKey,
    exit_edge: JsEdgeKey,
}

impl JsFlowTestPlan {
    pub(super) const fn new(
        value_flow: JsValueFlowPlan,
        body_edge: JsEdgeKey,
        exit_edge: JsEdgeKey,
    ) -> Self {
        Self {
            value_flow,
            body_edge,
            exit_edge,
        }
    }

    pub(crate) const fn value_flow(&self) -> &JsValueFlowPlan {
        &self.value_flow
    }

    pub(crate) const fn body_edge(&self) -> JsEdgeKey {
        self.body_edge
    }

    pub(crate) const fn exit_edge(&self) -> JsEdgeKey {
        self.exit_edge
    }
}

impl JsValueFlowPlan {
    pub(super) fn new(
        root: JsValueFlowStep,
        result_block: BlockId,
        result: ValueId,
        edges: Box<[JsEdgeKey]>,
    ) -> Self {
        Self {
            root,
            result_block,
            result,
            edges,
        }
    }

    pub(crate) const fn root(&self) -> &JsValueFlowStep {
        &self.root
    }

    pub(crate) const fn result_block(&self) -> BlockId {
        self.result_block
    }

    pub(crate) const fn result(&self) -> ValueId {
        self.result
    }

    pub(crate) const fn edges(&self) -> &[JsEdgeKey] {
        &self.edges
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum JsValueFlowStep {
    /// The branch has reached the flow's result block.
    Complete,

    /// Emit one block, then follow its planned terminator.
    Block {
        block: BlockId,
        continuation: JsValueFlowContinuation,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum JsValueFlowContinuation {
    /// Apply one SSA edge transfer and continue.
    Jump {
        edge: JsEdgeKey,
        next: Box<JsValueFlowStep>,
    },
    /// Select one of two acyclic continuations.
    Branch {
        condition: ValueId,
        then_edge: JsEdgeKey,
        then_step: Box<JsValueFlowStep>,
        else_edge: JsEdgeKey,
        else_step: Box<JsValueFlowStep>,
    },
}

#[derive(Debug)]
pub(crate) struct JsCatchPlan {
    exception: ValueId,
    body: JsControlSequence,
}

impl JsCatchPlan {
    pub(crate) const fn new(exception: ValueId, body: JsControlSequence) -> Self {
        Self { exception, body }
    }

    pub(crate) const fn exception(&self) -> ValueId {
        self.exception
    }

    pub(crate) const fn body(&self) -> &JsControlSequence {
        &self.body
    }
}

#[derive(Debug)]
pub(crate) struct JsTryPlan {
    try_body: JsControlSequence,
    catch: Option<JsCatchPlan>,
    finally: Option<JsControlSequence>,
}

impl JsTryPlan {
    pub(crate) const fn new(
        try_body: JsControlSequence,
        catch: Option<JsCatchPlan>,
        finally: Option<JsControlSequence>,
    ) -> Self {
        Self {
            try_body,
            catch,
            finally,
        }
    }

    pub(crate) const fn try_body(&self) -> &JsControlSequence {
        &self.try_body
    }

    pub(crate) const fn catch(&self) -> Option<&JsCatchPlan> {
        self.catch.as_ref()
    }

    pub(crate) const fn finally(&self) -> Option<&JsControlSequence> {
        self.finally.as_ref()
    }
}

#[derive(Debug)]
pub(crate) struct JsSwitchCasePlan {
    test_region: Option<RegionId>,
    entry_edge: JsEdgeKey,
    body: JsControlSequence,
}

impl JsSwitchCasePlan {
    pub(crate) const fn new(
        test_region: Option<RegionId>,
        entry_edge: JsEdgeKey,
        body: JsControlSequence,
    ) -> Self {
        Self {
            test_region,
            entry_edge,
            body,
        }
    }

    pub(crate) const fn test_region(&self) -> Option<RegionId> {
        self.test_region
    }

    pub(crate) const fn entry_edge(&self) -> JsEdgeKey {
        self.entry_edge
    }

    pub(crate) const fn body(&self) -> &JsControlSequence {
        &self.body
    }
}

#[derive(Debug)]
pub(crate) struct JsSwitchPlan {
    labels: Box<[Box<str>]>,
    discriminant: ValueId,
    matched_flag: Option<JsLocalId>,
    cases: Box<[JsSwitchCasePlan]>,
    no_match_edge: Option<JsEdgeKey>,
}

impl JsSwitchPlan {
    pub(crate) fn new(
        labels: Box<[Box<str>]>,
        discriminant: ValueId,
        matched_flag: Option<JsLocalId>,
        cases: Box<[JsSwitchCasePlan]>,
        no_match_edge: Option<JsEdgeKey>,
    ) -> Self {
        Self {
            labels,
            discriminant,
            matched_flag,
            cases,
            no_match_edge,
        }
    }

    pub(crate) fn labels(&self) -> &[Box<str>] {
        &self.labels
    }

    pub(crate) const fn discriminant(&self) -> ValueId {
        self.discriminant
    }

    pub(crate) const fn matched_flag(&self) -> Option<JsLocalId> {
        self.matched_flag
    }

    pub(crate) fn cases(&self) -> &[JsSwitchCasePlan] {
        &self.cases
    }

    pub(crate) const fn no_match_edge(&self) -> Option<JsEdgeKey> {
        self.no_match_edge
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JsIteratorKind {
    /// JavaScript `for (… in …)`.
    In,

    /// JavaScript `for (… of …)`.
    Of,

    /// JavaScript `for await (… of …)`.
    AwaitOf,
}

#[derive(Debug)]
pub(crate) struct JsIteratorPlan {
    kind: JsIteratorKind,
    labels: Box<[Box<str>]>,
    iterated_value: ValueId,
    produced_local: JsLocalId,
    completion_flag: JsLocalId,
    body: JsControlSequence,
    natural_exit_edge: JsEdgeKey,
}

impl JsIteratorPlan {
    pub(crate) const fn kind(&self) -> JsIteratorKind {
        self.kind
    }

    pub(crate) fn labels(&self) -> &[Box<str>] {
        &self.labels
    }

    pub(crate) const fn iterated_value(&self) -> ValueId {
        self.iterated_value
    }

    pub(crate) const fn produced_local(&self) -> JsLocalId {
        self.produced_local
    }

    pub(crate) const fn completion_flag(&self) -> JsLocalId {
        self.completion_flag
    }

    pub(crate) const fn body(&self) -> &JsControlSequence {
        &self.body
    }

    pub(crate) const fn natural_exit_edge(&self) -> JsEdgeKey {
        self.natural_exit_edge
    }
}

/// One validated classical `for` loop.
#[derive(Debug)]
pub(crate) struct JsForPlan {
    labels: Box<[Box<str>]>,
    initializer: JsControlSequence,
    initializer_is_prelude: bool,
    enter_test_edge: JsEdgeKey,
    test: JsForTestPlan,
    body: JsControlSequence,
    update: JsControlSequence,
}

impl JsForPlan {
    pub(crate) fn labels(&self) -> &[Box<str>] {
        &self.labels
    }

    pub(crate) const fn initializer(&self) -> &JsControlSequence {
        &self.initializer
    }

    pub(crate) const fn initializer_is_prelude(&self) -> bool {
        self.initializer_is_prelude
    }

    pub(crate) const fn enter_test_edge(&self) -> JsEdgeKey {
        self.enter_test_edge
    }

    pub(crate) const fn test(&self) -> &JsForTestPlan {
        &self.test
    }

    pub(crate) const fn body(&self) -> &JsControlSequence {
        &self.body
    }

    pub(crate) const fn update(&self) -> &JsControlSequence {
        &self.update
    }
}

/// The canonical test phase of one classical `for` loop.
#[derive(Debug)]
pub(crate) enum JsForTestPlan {
    /// An unconditional loop whose test block only transfers to the body.
    Always {
        block: BlockId,
        body_edge: JsEdgeKey,
    },
    /// A test contained in one block.
    Conditional {
        block: BlockId,
        condition: ValueId,
        body_edge: JsEdgeKey,
        exit_edge: JsEdgeKey,
    },
    /// An acyclic test expression spanning multiple IR blocks.
    Flow {
        value_flow: JsValueFlowPlan,
        body_edge: JsEdgeKey,
        exit_edge: JsEdgeKey,
    },
}
