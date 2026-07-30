//! Transformations over Evrel IR.

mod eliminate_common_subexpressions;
mod eliminate_dead_code;
mod promote_bindings_to_ssa;
mod propagate_constants;
mod prune_unreachable_blocks;
mod simplify_block_parameters;
mod simplify_control_flow;
mod simplify_operations;
mod ssa_updater;

pub use eliminate_common_subexpressions::eliminate_common_subexpressions;
pub use eliminate_dead_code::eliminate_dead_code;
pub use promote_bindings_to_ssa::promote_bindings_to_ssa;
pub use propagate_constants::propagate_constants;
pub use prune_unreachable_blocks::prune_unreachable_blocks;
pub use simplify_block_parameters::simplify_block_parameters;
pub use simplify_control_flow::simplify_control_flow;
pub use simplify_operations::simplify_operations;
