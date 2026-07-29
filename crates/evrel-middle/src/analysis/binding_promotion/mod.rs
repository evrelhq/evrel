//! Binding-promotion eligibility analysis.

mod function;
mod module;

pub use function::{FunctionBindingPromotion, PromotableBinding};
pub use module::ModuleBindingPromotion;
