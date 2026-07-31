//! Public facade for function-local value analysis.

use evrel_js_ir::{BlockId, JsFunctionIr, OperationId, ValueId};

use crate::analysis::RegionControlFlowError;

use super::sparse::SparseValueAnalysis;
use super::{AbstractValue, FunctionValueInputs};

/// Immutable value facts for one function snapshot.
///
/// The analysis covers every region owned by the function. Recompute it after
/// changing operations, operands, block parameters, control flow, regions, or
/// the function signature.
#[derive(Debug)]
pub struct FunctionValueAnalysis {
    sparse: SparseValueAnalysis,
}

impl FunctionValueAnalysis {
    /// Computes value facts with unknown external inputs.
    pub fn compute(function: &JsFunctionIr) -> Result<Self, RegionControlFlowError> {
        Self::compute_with_inputs(function, &FunctionValueInputs::new())
    }

    /// Computes value facts with explicitly supplied boundary or result facts.
    pub fn compute_with_inputs(
        function: &JsFunctionIr,
        inputs: &FunctionValueInputs,
    ) -> Result<Self, RegionControlFlowError> {
        Ok(Self {
            sparse: SparseValueAnalysis::compute(function, inputs)?,
        })
    }

    /// Returns the context-independent fact for an SSA value.
    pub fn value(&self, value: ValueId) -> &AbstractValue {
        self.sparse.value(value)
    }

    /// Returns whether sparse execution reaches a block.
    ///
    /// Each region entry is analyzed as an independent execution boundary.
    pub fn is_block_executable(&self, block: BlockId) -> bool {
        self.sparse.is_block_executable(block)
    }

    /// Returns whether an ordinary successor edge is executable.
    pub fn is_edge_executable(&self, terminator: OperationId, successor_index: usize) -> bool {
        self.sparse.is_edge_executable(terminator, successor_index)
    }

    /// Returns the sole executable successor, when exactly one exists.
    ///
    /// This proves edge feasibility only. Transformations remain responsible
    /// for preserving operation-specific structured semantics.
    pub fn unique_executable_successor(
        &self,
        function: &JsFunctionIr,
        terminator: OperationId,
    ) -> Option<usize> {
        let successor_count = function.operation(terminator)?.successors().len();
        let mut executable =
            (0..successor_count).filter(|index| self.is_edge_executable(terminator, *index));

        let successor = executable.next()?;

        executable.next().is_none().then_some(successor)
    }
}
