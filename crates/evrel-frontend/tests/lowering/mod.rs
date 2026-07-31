use evrel_js_ir::{
    BindingKind, BlockParameterSource, CompilerLocation, ExceptionHandlerKind, ForOfKind,
    FunctionKind, FunctionMode, JsModuleIr, LoopKind, OperationKind, TextRange, print_function,
    print_module,
};

use evrel_frontend::{FrontendError, lower_source_file};

fn lower_javascript_module(source: &str) -> Result<JsModuleIr, FrontendError> {
    lower_source_file("input.mjs", source)
}

fn lower_typescript_module(source: &str) -> Result<JsModuleIr, FrontendError> {
    lower_source_file("input.ts", source)
}

fn print_entry_function(module: &JsModuleIr) -> String {
    let function = module
        .function(module.entry_function())
        .expect("entry function must remain live");

    print_function(function)
}

mod bindings;
mod classes;
mod control_flow;
mod exceptions;
mod expressions;
mod functions;
mod literals;
mod logical;
mod modules;
mod objects;
mod parameters;
mod source;
mod suspension;
mod switch;
mod typescript;
