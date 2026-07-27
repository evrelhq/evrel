//! Source-type and strictness lowering.

use super::*;

#[test]
fn infers_source_type_from_the_source_name() {
    assert!(lower_source_file("input.jsx", "const value = 42;").is_ok());
    assert!(lower_source_file("input.tsx", "const value: number = 42;").is_ok());
}

#[test]
fn retains_implicit_strictness_in_the_ir() {
    let module =
        lower_source_file("input.mjs", "export function read(value) { return value; }").unwrap();
    let function = module
        .functions()
        .find_map(|(_, function)| (function.kind() == FunctionKind::Ordinary).then_some(function))
        .unwrap();

    assert!(function.is_strict());

    let module = lower_source_file("input.cjs", "function read(value) { return value; }").unwrap();
    let function = module
        .functions()
        .find_map(|(_, function)| (function.kind() == FunctionKind::Ordinary).then_some(function))
        .unwrap();

    assert!(!function.is_strict());
}

#[test]
fn retains_explicit_strictness_and_its_directive_provenance() {
    let module = lower_source_file(
        "input.cjs",
        r#"
                "use strict";
                function inherited(value) { return value; }
                function explicit(value) { "use strict"; return value; }
            "#,
    )
    .unwrap();
    let entry = module.function(module.entry_function()).unwrap();
    let ordinary = module
        .functions()
        .filter(|(_, function)| function.kind() == FunctionKind::Ordinary)
        .map(|(_, function)| function)
        .collect::<Vec<_>>();

    assert!(entry.is_strict());
    assert!(entry.has_use_strict_directive());
    assert!(ordinary.iter().all(|function| function.is_strict()));
    assert_eq!(
        ordinary
            .iter()
            .filter(|function| function.has_use_strict_directive())
            .count(),
        1
    );
}

#[test]
fn marks_class_execution_contexts_strict_in_sloppy_sources() {
    let module = lower_source_file(
        "input.cjs",
        "class Example { field = 1; method(value) { return value; } static { this.x = 1; } }",
    )
    .unwrap();

    assert!(
        module
            .functions()
            .filter(|(_, function)| {
                matches!(
                    function.kind(),
                    FunctionKind::ClassMethod
                        | FunctionKind::ClassFieldInitializer
                        | FunctionKind::ClassStaticBlock
                )
            })
            .all(|(_, function)| function.is_strict())
    );
}

#[test]
fn rejects_unknown_source_extensions() {
    assert!(matches!(
        lower_source_file("input.txt", "20 + 22;"),
        Err(super::FrontendError::UnknownSourceType { .. }),
    ));
}

#[test]
fn lowers_structured_jsx_elements_and_fragments() {
    let module = lower_source_file(
        "input.jsx",
        r#"<div enabled title="Save" {...props}>Hello {name}<></></div>;"#,
    )
    .unwrap();
    let output = print_entry_function(&module);

    assert!(output.contains("jsx_element intrinsic \"div\""));
    assert!(output.contains("\"enabled\""));
    assert!(output.contains("\"title\"=\"Save\""));
    assert!(output.contains("...region @"));
    assert!(output.contains("text \"Hello \""));
    assert!(output.contains("expression region @"));
    assert!(output.contains("fragment region @"));
    assert!(output.contains("jsx_fragment children: []"));
}

#[test]
fn lowers_jsx_component_member_names_as_references() {
    let module = lower_source_file(
        "input.jsx",
        "const UI = {}; <UI.Button.Primary />; <this.Item />;",
    )
    .unwrap();
    let output = print_entry_function(&module);

    assert!(output.contains("load_binding @0"));
    assert!(output.contains("jsx_element member %"));
    assert!(output.contains(".Button.Primary"));
    assert!(output.contains("jsx_element member this.Item"));
}
