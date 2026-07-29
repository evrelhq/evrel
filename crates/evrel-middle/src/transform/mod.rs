//! Transformations over Evrel IR.

mod promote_bindings_to_ssa;
mod propagate_constants;
mod ssa_updater;

pub use promote_bindings_to_ssa::promote_bindings_to_ssa;
pub use propagate_constants::propagate_constants;
