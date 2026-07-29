//! Propagation of constants over function-local SSA values.

mod rewrite;

use evrel_ir::ModuleIr;

use crate::analysis::FunctionValueAnalysis;

use rewrite::{plan_constant_replacements, rewrite_constants};

/// Replaces proven effect-free operation results with exact constants.
///
/// Functions whose control flow cannot yet be modeled soundly are left
/// unchanged. Returns the number of operations replaced.
pub fn propagate_constants(module: &mut ModuleIr) -> usize {
    let mut replaced = 0;

    for (_, function) in module.functions_mut() {
        let replacements = {
            let Ok(analysis) = FunctionValueAnalysis::compute(function) else {
                continue;
            };

            plan_constant_replacements(function, &analysis)
        };

        replaced += rewrite_constants(function, replacements);
    }

    replaced
}

#[cfg(test)]
mod tests;
