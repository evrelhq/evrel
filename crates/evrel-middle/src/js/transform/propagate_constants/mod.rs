//! Propagation of constants over function-local SSA values.

mod rewrite;

use evrel_js_ir::JsFunctionIr;

use crate::js::analysis::FunctionValueAnalysis;

use rewrite::{plan_constant_replacements, rewrite_constants};

/// Replaces proven effect-free operation results with exact constants.
///
pub fn propagate_constants(function: &mut JsFunctionIr) -> usize {
    let replacements = {
        let analysis = FunctionValueAnalysis::compute(function);

        plan_constant_replacements(function, &analysis)
    };

    rewrite_constants(function, replacements)
}

#[cfg(test)]
mod tests;
