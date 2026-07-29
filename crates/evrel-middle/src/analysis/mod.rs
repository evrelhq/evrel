//! Read-only computations over Evrel IR.

mod binding_promotion;
mod control_flow;
mod dominance_frontier;
mod dominator_tree;
mod function_value;

pub use binding_promotion::{FunctionBindingPromotion, ModuleBindingPromotion, PromotableBinding};
pub use control_flow::{RegionControlFlowError, RegionControlFlowGraph};
pub use dominance_frontier::RegionDominanceFrontier;
pub use dominator_tree::RegionDominatorTree;
pub use function_value::{AbstractValue, FunctionValueAnalysis, FunctionValueInputs};
