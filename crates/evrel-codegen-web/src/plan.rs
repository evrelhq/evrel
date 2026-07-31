//! Whole-program orchestration for web output.
//!
//! This module composes language-specific output plans. It owns no framework
//! semantics and performs no optimization.

use evrel_js_ir::{JsProgramIr, ModuleId};
use thiserror::Error;

use crate::{JsCodegenError, JsModulePlan};

/// JavaScript output selected for one program module.
#[derive(Debug)]
pub struct WebModulePlan {
    module: ModuleId,
    javascript: JsModulePlan,
}

impl WebModulePlan {
    /// Returns the source program module represented by this plan.
    pub const fn module(&self) -> ModuleId {
        self.module
    }

    /// Returns the finalized JavaScript plan for the module.
    pub const fn javascript(&self) -> &JsModulePlan {
        &self.javascript
    }
}

/// Complete output plan for one web program.
///
/// The module boundary is retained so HTML and CSS plans can be added without
/// turning the JavaScript planner into a bundler or framework layer.
#[derive(Debug)]
pub struct WebOutputPlan {
    modules: Box<[WebModulePlan]>,
}

impl WebOutputPlan {
    /// Plans every retained JavaScript module in stable program order.
    pub fn build(program: &JsProgramIr) -> Result<Self, WebPlanError> {
        let mut modules = Vec::with_capacity(program.modules().count());

        for (module, program_module) in program.modules() {
            let javascript = crate::js::plan(program_module.ir())
                .map_err(|source| WebPlanError { module, source })?;
            modules.push(WebModulePlan { module, javascript });
        }

        Ok(Self {
            modules: modules.into_boxed_slice(),
        })
    }

    /// Iterates over module plans in stable program order.
    pub fn modules(&self) -> impl ExactSizeIterator<Item = &WebModulePlan> {
        self.modules.iter()
    }

    /// Returns the output plan for `module`.
    pub fn module(&self, module: ModuleId) -> Option<&WebModulePlan> {
        self.modules
            .binary_search_by_key(&module, WebModulePlan::module)
            .ok()
            .map(|index| &self.modules[index])
    }
}

/// Failure while constructing a web output plan.
#[derive(Debug, Error)]
#[error("failed to plan JavaScript module {module:?}: {source}")]
pub struct WebPlanError {
    module: ModuleId,
    #[source]
    source: JsCodegenError,
}

impl WebPlanError {
    /// Returns the module whose output could not be planned.
    pub const fn module(&self) -> ModuleId {
        self.module
    }
}

#[cfg(test)]
mod tests {
    use evrel_frontend::lower_source_file;
    use evrel_js_ir::{JsProgramIr, ModuleKey};

    use super::WebOutputPlan;

    #[test]
    fn plans_every_program_module_in_stable_order() {
        let mut program = JsProgramIr::new();
        let first = program.add_module(
            ModuleKey::new("file:///first.js"),
            lower_source_file("first.js", "export const first = 1;").unwrap(),
        );
        let second = program.add_module(
            ModuleKey::new("file:///second.js"),
            lower_source_file("second.js", "export const second = 2;").unwrap(),
        );

        let plan = WebOutputPlan::build(&program).unwrap();
        let modules = plan
            .modules()
            .map(|module| module.module())
            .collect::<Vec<_>>();

        assert_eq!(modules, [first, second]);
        assert!(plan.module(first).is_some());
        assert!(plan.module(second).is_some());
    }
}
