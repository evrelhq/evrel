//! Context-independent sparse conditional value analysis.

use evrel_js_ir::{BlockId, BlockParameterSource, JsFunctionIr, OperationId, RegionId, ValueId};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::js::analysis::{RegionControlFlowError, RegionControlFlowGraph};
use crate::js::work_queue::WorkQueue;

use super::abstract_value::BOTTOM;
use super::transfer::{constant_truthiness, evaluate_result};
use super::{AbstractValue, FunctionValueInputs};

/// Context-independent value facts and executable control flow.
#[derive(Debug)]
pub(super) struct SparseValueAnalysis {
    values: FxHashMap<ValueId, AbstractValue>,
    executable_blocks: FxHashSet<BlockId>,
    executable_edges: FxHashSet<(OperationId, usize)>,
}

impl SparseValueAnalysis {
    pub(super) fn compute(
        function: &JsFunctionIr,
        inputs: &FunctionValueInputs,
    ) -> Result<Self, RegionControlFlowError> {
        Ok(SparseValueSolver::new(function, inputs)?.solve())
    }

    pub(super) fn value(&self, value: ValueId) -> &AbstractValue {
        self.values
            .get(&value)
            .unwrap_or_else(|| unreachable!("analysis has no fact for value {value:?}"))
    }

    pub(super) fn is_block_executable(&self, block: BlockId) -> bool {
        self.executable_blocks.contains(&block)
    }

    pub(super) fn is_edge_executable(
        &self,
        terminator: OperationId,
        successor_index: usize,
    ) -> bool {
        self.executable_edges
            .contains(&(terminator, successor_index))
    }
}

struct SparseValueSolver<'ir, 'inputs> {
    function: &'ir JsFunctionIr,
    inputs: &'inputs FunctionValueInputs,
    control_flow: FxHashMap<RegionId, RegionControlFlowGraph>,
    values: FxHashMap<ValueId, AbstractValue>,
    executable_blocks: FxHashSet<BlockId>,
    executable_edges: FxHashSet<(OperationId, usize)>,
    visited_blocks: FxHashSet<BlockId>,
    scanning_block: Option<BlockId>,
    block_work: WorkQueue<BlockId>,
    operation_work: WorkQueue<OperationId>,
}

impl<'ir, 'inputs> SparseValueSolver<'ir, 'inputs> {
    fn new(
        function: &'ir JsFunctionIr,
        inputs: &'inputs FunctionValueInputs,
    ) -> Result<Self, RegionControlFlowError> {
        let control_flow = function
            .regions()
            .map(|(region, _)| {
                RegionControlFlowGraph::compute(function, region).map(|graph| (region, graph))
            })
            .collect::<Result<FxHashMap<_, _>, _>>()?;

        Ok(Self {
            function,
            inputs,
            control_flow,
            values: FxHashMap::default(),
            executable_blocks: FxHashSet::default(),
            executable_edges: FxHashSet::default(),
            visited_blocks: FxHashSet::default(),
            scanning_block: None,
            block_work: WorkQueue::new(),
            operation_work: WorkQueue::new(),
        })
    }

    fn solve(mut self) -> SparseValueAnalysis {
        self.initialize_boundaries();

        loop {
            if let Some(operation) = self.operation_work.pop() {
                self.visit_scheduled_operation(operation);
                continue;
            }

            if let Some(block) = self.block_work.pop() {
                self.visit_block(block);
                continue;
            }

            break;
        }

        // Every live value receives an explicit final fact. Values that were
        // never reached remain bottom.
        for (value, _) in self.function.values() {
            self.values
                .entry(value)
                .or_insert_with(AbstractValue::bottom);
        }

        SparseValueAnalysis {
            values: self.values,
            executable_blocks: self.executable_blocks,
            executable_edges: self.executable_edges,
        }
    }

    fn initialize_boundaries(&mut self) {
        for parameter in self.function.parameters() {
            let value = parameter.value();
            let fact = self
                .inputs
                .boundary_value(value)
                .cloned()
                .unwrap_or_else(AbstractValue::unknown);

            self.values.insert(value, fact);
        }

        let entries = self
            .control_flow
            .values()
            .map(RegionControlFlowGraph::entry)
            .collect::<Vec<_>>();

        for entry in entries {
            let parameters = self
                .function
                .block(entry)
                .expect("region entry must remain live")
                .parameters()
                .to_vec();

            for parameter in parameters {
                let value = parameter.value();
                let fact = self
                    .inputs
                    .boundary_value(value)
                    .cloned()
                    .unwrap_or_else(AbstractValue::unknown);

                self.values.insert(value, fact);
            }

            self.mark_block_executable(entry);
        }
    }

    fn visit_block(&mut self, block: BlockId) {
        if !self.executable_blocks.contains(&block) {
            return;
        }

        assert!(
            self.visited_blocks.insert(block),
            "an executable block must be scanned as a whole only once",
        );

        let block_data = self
            .function
            .block(block)
            .expect("executable block must remain live");
        let operations = block_data.operations().to_vec();
        let terminator = block_data.terminator();

        self.scanning_block = Some(block);

        for operation in operations {
            self.visit_value_operation(operation);
        }

        self.scanning_block = None;

        if let Some(terminator) = terminator {
            self.visit_terminator(terminator);
        }
    }

    fn visit_scheduled_operation(&mut self, operation: OperationId) {
        let data = self
            .function
            .operation(operation)
            .expect("scheduled operation must remain live");
        let block = data.block();

        if !self.executable_blocks.contains(&block) || !self.visited_blocks.contains(&block) {
            return;
        }

        let is_terminator = self
            .function
            .block(block)
            .expect("operation block must remain live")
            .terminator()
            == Some(operation);

        if is_terminator {
            self.visit_terminator(operation);
        } else {
            self.visit_value_operation(operation);
        }
    }

    fn visit_value_operation(&mut self, operation: OperationId) {
        let data = self
            .function
            .operation(operation)
            .expect("executable operation must remain live");

        if data.results().is_empty() {
            return;
        }

        let operands = data
            .operands()
            .iter()
            .map(|operand| self.value(*operand).clone())
            .collect::<Vec<_>>();

        for (result_index, result) in data.results().iter().copied().enumerate() {
            let incoming = self
                .inputs
                .result_value(result)
                .cloned()
                .unwrap_or_else(|| evaluate_result(data.kind(), &operands, result_index));

            self.update_value(result, incoming);
        }
    }

    fn visit_terminator(&mut self, terminator: OperationId) {
        let data = self
            .function
            .operation(terminator)
            .expect("executable terminator must remain live");
        let successors = data.successors();

        if successors.is_empty() {
            return;
        }

        let executable_successors = if data.kind().is_conditional_branch() {
            match self.value(data.operands()[0]) {
                value if value.is_bottom() => Vec::new(),

                value => match value.constant() {
                    Some(condition) => {
                        vec![usize::from(!constant_truthiness(condition))]
                    }

                    None => (0..successors.len()).collect(),
                },
            }
        } else {
            (0..successors.len()).collect()
        };

        for successor_index in executable_successors {
            self.mark_edge_executable(terminator, successor_index);
        }
    }

    fn mark_edge_executable(&mut self, terminator: OperationId, successor_index: usize) {
        let successor = self
            .function
            .operation(terminator)
            .expect("edge terminator must remain live")
            .successors()
            .get(successor_index)
            .copied()
            .expect("terminator must contain the selected successor");

        self.executable_edges.insert((terminator, successor_index));

        let target = successor.target().block();

        self.mark_block_executable(target);
        self.recompute_block_parameters(target);
    }

    fn recompute_block_parameters(&mut self, block: BlockId) {
        let parameters = self
            .function
            .block(block)
            .expect("edge target must remain live")
            .parameters()
            .to_vec();

        if parameters.is_empty() {
            return;
        }

        let region = self
            .function
            .block_region(block)
            .expect("edge target must belong to a region");
        let incoming_edges = self
            .control_flow
            .get(&region)
            .expect("target region must have a control-flow graph")
            .predecessor_edges(block)
            .expect("target block must belong to its regional graph")
            .iter()
            .map(|edge| {
                *self
                    .control_flow
                    .get(&region)
                    .expect("target region must have a control-flow graph")
                    .edge(*edge)
                    .expect("incoming edge must remain live")
            })
            .collect::<Vec<_>>();

        let mut incoming_values = vec![AbstractValue::bottom(); parameters.len()];

        for edge in incoming_edges {
            if !self
                .executable_edges
                .contains(&(edge.terminator(), edge.successor_index()))
            {
                continue;
            }

            let operation = self
                .function
                .operation(edge.terminator())
                .expect("incoming terminator must remain live");
            let successor = operation.successors()[edge.successor_index()];
            let produced_count = successor.produced_argument_count();
            let forwarded = successor.arguments(operation.operands());

            for (parameter_index, parameter) in parameters.iter().enumerate() {
                let incoming = match parameter.source() {
                    BlockParameterSource::Produced if parameter_index < produced_count => {
                        AbstractValue::unknown()
                    }

                    BlockParameterSource::Forwarded if parameter_index >= produced_count => self
                        .value(forwarded[parameter_index - produced_count])
                        .clone(),

                    // Exceptional transfer is not represented by an ordinary
                    // regional edge. Produced and forwarded mismatches indicate
                    // malformed IR, but conservatively remain unknown here.
                    BlockParameterSource::Exception
                    | BlockParameterSource::Produced
                    | BlockParameterSource::Forwarded => AbstractValue::unknown(),
                };

                incoming_values[parameter_index] = incoming_values[parameter_index].join(&incoming);
            }
        }

        for (parameter, incoming) in parameters.into_iter().zip(incoming_values) {
            self.update_value(parameter.value(), incoming);
        }
    }

    fn update_value(&mut self, value: ValueId, incoming: AbstractValue) {
        let current = self
            .values
            .get(&value)
            .cloned()
            .unwrap_or_else(AbstractValue::bottom);
        let next = current.join(&incoming);

        if current == next {
            return;
        }

        self.values.insert(value, next);

        let users = self
            .function
            .value(value)
            .expect("updated value must remain live")
            .uses()
            .iter()
            .map(|use_site| use_site.operation())
            .collect::<Vec<_>>();

        for operation in users {
            self.schedule_operation(operation);
        }
    }

    fn value(&self, value: ValueId) -> &AbstractValue {
        self.values.get(&value).unwrap_or(&BOTTOM)
    }

    fn mark_block_executable(&mut self, block: BlockId) {
        if self.executable_blocks.insert(block) {
            self.block_work.push(block);
        }
    }

    fn schedule_operation(&mut self, operation: OperationId) {
        let block = self
            .function
            .operation(operation)
            .expect("value user must remain live")
            .block();

        // Definitions precede their same-block users. The current whole-block
        // scan will reach those users without separately scheduling them.
        if self.scanning_block == Some(block) {
            return;
        }

        if self.executable_blocks.contains(&block) && self.visited_blocks.contains(&block) {
            self.operation_work.push(operation);
        }
    }
}
