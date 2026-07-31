//! Planning and emission for web output.

pub mod js;
mod plan;

pub use js::{JsCodegenError, JsModulePlan, emit, generate, plan};
pub use plan::{WebModulePlan, WebOutputPlan, WebPlanError};
