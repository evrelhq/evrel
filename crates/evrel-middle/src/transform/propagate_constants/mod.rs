//! Propagation of constants over function-local SSA values.

mod rewrite;

use evrel_ir::FunctionIr;

use crate::analysis::FunctionValueAnalysis;

use rewrite::{plan_constant_replacements, rewrite_constants};

/// Replaces proven effect-free operation results with exact constants.
///
/// Returns zero when the function's value flow cannot be modeled soundly.
pub fn propagate_constants(function: &mut FunctionIr) -> usize {
    let replacements = {
        let Ok(analysis) = FunctionValueAnalysis::compute(function) else {
            return 0;
        };

        plan_constant_replacements(function, &analysis)
    };

    rewrite_constants(function, replacements)
}

#[cfg(test)]
mod tests;
