mod support;

use evrel_codegen_web::generate;
use evrel_frontend::lower_source_file;
use evrel_js_ir::JsModuleIr;

use support::{assert_same_result, generate_source};

#[test]
fn empty_module_maps_to_empty_output() {
    assert_eq!(generate(&JsModuleIr::new()).unwrap(), "");
}

#[test]
fn mapping_is_deterministic() {
    let source = "export function add(left, right) { return left + right; }";
    let module = lower_source_file("input.js", source).unwrap();

    assert_eq!(generate(&module).unwrap(), generate(&module).unwrap(),);
}

#[test]
fn representative_ir_maps_to_parseable_javascript() {
    generate_source(
        r#"
        export async function run(input) {
            const values = [input, ...input.items];
            const object = {
                value: values[0],
                method(argument) { return argument?.value ?? this.value; },
            };
            for (const value of values) {
                if (value) object.value += await object.method(value);
            }
            return object.value;
        }
        "#,
    );
}

#[test]
fn generated_locals_do_not_shadow_referenced_globals() {
    let output = generate_source(
        r#"
        const value = globalThis.value;
        console.log(value, Promise.resolve(value));
        "#,
    );

    for global in ["globalThis", "console", "Promise"] {
        assert!(
            !output.contains(&format!("let {global}")),
            "generated local shadowed `{global}`:\n{}",
            output,
        );
    }
}

#[test]
fn control_flow_is_emitted_explicitly() {
    let output = generate_source(
        r#"
        export function choose(condition, left, right) {
            if (condition) return left;
            return right;
        }
        "#,
    );

    assert!(output.contains("if ("));
    assert!(output.contains("return "));
}

#[test]
fn loop_phases_remain_native_control_structure() {
    let output = generate_source(
        r#"
        export function sum(values) {
            let total = 0;
            for (let index = 0; index < values.length; index++) {
                total += values[index];
            }
            return total;
        }
        "#,
    );

    assert!(output.contains("for ("));
}

#[test]
fn calls_preserve_member_receiver_syntax() {
    let output = generate_source("export const result = object.method(argument);");

    assert!(output.contains(".method("));
}

#[test]
fn direct_eval_keeps_the_callers_lexical_environment() {
    assert_same_result(
        r#"
        function read() {
            const local = 41;
            console.log(eval("local + 1"));
        }
        read();
        "#,
    );
}

#[test]
fn single_use_function_and_class_creations_preserve_behavior() {
    assert_same_result(
        r#"
        console.log(
            (function (value) { return value + 1; })(41),
            new (class { value = 42 })().value,
        );
        "#,
    );
}

#[test]
fn representative_control_flow_matches_source_execution() {
    assert_same_result(
        r#"
        const values = [1, 2, 3, 4];
        let total = 0;
        for (const value of values) {
            if (value === 2) continue;
            total += value;
            if (total > 6) break;
        }
        try {
            throw total;
        } catch (value) {
            console.log(value);
        } finally {
            console.log("done");
        }
        "#,
    );
}
