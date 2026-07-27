//! JavaScript backend for Evrel IR.

mod emit;
mod error;
mod generate;
mod name;
mod plan;

pub use error::JsCodegenError;
pub use generate::generate;
