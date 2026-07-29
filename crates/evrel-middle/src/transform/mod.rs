//! Transformations over Evrel IR.

mod promote_bindings_to_ssa;
mod ssa_updater;

pub use promote_bindings_to_ssa::promote_bindings_to_ssa;
