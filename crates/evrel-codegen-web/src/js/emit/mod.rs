//! Evrel IR to Oxc AST emission.

mod array;
mod binary;
mod binding;
mod call;
mod class;
mod completion;
mod constant;
mod context;
mod control;
mod delete;
mod destructure;
mod function;
mod global;
mod jsx;
mod module;
mod object;
mod operand;
mod operation;
mod pattern;
mod predicate;
mod property;
mod regexp;
mod region;
mod sequence;
mod suspension;
mod template;
mod unary;
mod update;
mod value;

use evrel_js_ir::{JsFunctionIr, JsModuleIr};
use oxc_ast::AstBuilder;

use crate::js::plan::{JsFunctionPlan, JsModulePlan};

#[derive(Clone, Copy)]
pub(crate) struct FunctionEmission<'emit, 'ast> {
    builder: &'emit AstBuilder<'ast>,
    module: &'emit JsModuleIr,
    output_plan: &'emit JsModulePlan,
    function: &'emit JsFunctionIr,
    plan: &'emit JsFunctionPlan,
}

impl<'emit, 'ast> FunctionEmission<'emit, 'ast> {
    pub(crate) const fn new(
        builder: &'emit AstBuilder<'ast>,
        module: &'emit JsModuleIr,
        output_plan: &'emit JsModulePlan,
        function: &'emit JsFunctionIr,
        plan: &'emit JsFunctionPlan,
    ) -> Self {
        Self {
            builder,
            module,
            output_plan,
            function,
            plan,
        }
    }
}

pub(crate) use function::{emit_function_body, emit_function_directives};
pub(crate) use module::{
    attach_local_export_declarations, emit_module_exports, emit_module_imports,
};
