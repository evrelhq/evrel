//! Transformations over Evrel IR.

mod eliminate_dead_code;
mod promote_bindings_to_ssa;
mod propagate_constants;
mod simplify_block_parameters;
mod ssa_updater;

pub use eliminate_dead_code::eliminate_dead_code;
pub use promote_bindings_to_ssa::promote_bindings_to_ssa;
pub use propagate_constants::propagate_constants;
pub use simplify_block_parameters::simplify_block_parameters;
