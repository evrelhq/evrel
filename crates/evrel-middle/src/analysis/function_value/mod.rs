//! Function-local analysis of JavaScript SSA values.
//!
//! A sparse conditional analysis will compute value facts and executable
//! control flow for every region owned by a function.

mod abstract_value;
mod analysis;
mod inputs;
mod sparse;
mod transfer;

pub use abstract_value::AbstractValue;
pub use analysis::FunctionValueAnalysis;
pub use inputs::FunctionValueInputs;

#[cfg(test)]
mod tests;
