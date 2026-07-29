//! Transformations over Evrel IR.

mod constant_propagation;
mod promote_bindings_to_ssa;
mod ssa_updater;

pub use constant_propagation::propagate_constants;
pub use promote_bindings_to_ssa::promote_bindings_to_ssa;
