//! Backend-owned JavaScript output planning.

mod control;
mod dense;
mod edge;
mod function;
mod local;
mod operation;
mod region;
mod value;

use evrel_js_ir::{FunctionId, JsModuleIr};

use crate::{JsCodegenError, js::name::JsReservedNames};

pub(crate) use control::{
    JsControlPlan, JsControlSequence, JsControlStep, JsEdgeKey, JsForTestPlan, JsIteratorKind,
    JsSwitchPlan, JsTryPlan, JsValueFlowContinuation, JsValueFlowPlan, JsValueFlowStep,
};
pub(crate) use dense::DenseMap;
pub(crate) use edge::{JsEdgeTransfer, JsMoveSource, build_edge_transfers};
pub(crate) use function::JsFunctionPlan;
pub(crate) use local::{JsLocalAllocator, JsLocalId, JsNamePlan};
pub(crate) use operation::{JsOperationPlan, JsOperationStatementPlan};
pub(crate) use region::{
    JsExpressionRegionContinuation, JsExpressionRegionPlan, JsExpressionRegionStep,
};
pub(crate) use value::JsValueRepresentation;

/// Complete JavaScript emission plan for a module.
#[derive(Debug)]
pub(crate) struct JsModulePlan {
    functions: DenseMap<FunctionId, JsFunctionPlan>,
}

impl JsModulePlan {
    pub(crate) fn build(module: &JsModuleIr) -> Result<Self, JsCodegenError> {
        let reserved_names = JsReservedNames::collect(module);
        let mut functions = DenseMap::new();

        for (function_id, function) in module.functions() {
            let function_plan =
                JsFunctionPlan::build(module, function_id, function, &reserved_names)?;

            functions.insert(function_id, function_plan);
        }

        Ok(Self { functions })
    }

    pub(crate) fn function(&self, function: FunctionId) -> Option<&JsFunctionPlan> {
        self.functions.get(function)
    }
}
