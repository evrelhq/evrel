//! JavaScript backend orchestration.

use evrel_js_ir::JsModuleIr;
use oxc_allocator::{Allocator, Vec as ArenaVec};
use oxc_ast::{
    AstBuilder,
    ast::{Program, SourceType},
};
use oxc_codegen::Codegen;
use oxc_span::SPAN;

use crate::{
    JsCodegenError,
    js::emit::{
        attach_local_export_declarations, emit_function_body, emit_function_directives,
        emit_module_exports, emit_module_imports,
    },
    js::plan::JsModulePlan,
};

/// Generates JavaScript from verified Evrel IR.
pub fn generate(module: &JsModuleIr) -> Result<String, JsCodegenError> {
    let function_id = module.entry_function();
    let module_plan = JsModulePlan::build(module)?;

    let allocator = Allocator::default();
    let builder = AstBuilder::new(&allocator);

    let mut body = emit_module_imports(&builder, module, &module_plan)?;
    let module_body = emit_function_body(&builder, module, &module_plan, function_id)?;
    let (module_body, attached_exports) =
        attach_local_export_declarations(&builder, module, &module_plan, module_body);
    body.extend(module_body);
    body.extend(emit_module_exports(
        &builder,
        module,
        &module_plan,
        &attached_exports,
    )?);

    let program = Program::new(
        SPAN,
        SourceType::mjs(),
        "",
        ArenaVec::new_in(&builder),
        None,
        emit_function_directives(
            &builder,
            module
                .function(function_id)
                .expect("the module entry function must remain live"),
        ),
        body,
        &builder,
    );
    let code = Codegen::new().build(&program).code;

    Ok(code)
}

#[cfg(test)]
mod tests {
    use evrel_frontend::lower_source_file;

    use super::generate;

    #[test]
    fn generates_deterministically_across_rebuilt_module_plans() {
        let module = lower_source_file(
            "input.js",
            r#"
                const events = [];
                const target = { value: 1 };
                function left() {
                    events.push("left");
                    return 2;
                }
                function right() {
                    events.push("right");
                    return 3;
                }
                const assigned =
                    target.value += events.length === 0 ? left() : right();
                let index = 0;
                for (; index < assigned; index++) {
                    target.value ||= index;
                }
                export { target, assigned };
            "#,
        )
        .unwrap();
        let expected = generate(&module).unwrap();

        for _ in 0..32 {
            assert_eq!(generate(&module).unwrap(), expected);
        }
    }
}
