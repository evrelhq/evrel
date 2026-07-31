//! JavaScript planning and emission for web output.

mod emit;
mod error;
mod generate;
mod name;
mod plan;

pub use error::JsCodegenError;
pub use generate::{emit, generate, plan};
pub use plan::JsModulePlan;
