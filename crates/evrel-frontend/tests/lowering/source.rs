//! Source-type and strictness lowering.

use super::*;

#[test]
fn infers_source_type_from_the_source_name() {
    assert!(lower_source_file("input.jsx", "const value = 42;").is_ok());
    assert!(lower_source_file("input.tsx", "const value: number = 42;").is_ok());
}

#[test]
fn records_exact_source_ranges_for_lowered_operations() {
    let source = "console[20 + 22];";
    let module = lower_source_file("input.mjs", source).unwrap();
    let function = module.function(module.entry_function()).unwrap();

    let ranges = function
        .operations()
        .filter_map(|(_, operation)| {
            let CompilerLocation::Source { file, range } =
                module.location(operation.location()).unwrap()
            else {
                panic!("frontend operations must have concrete source locations");
            };

            match operation.kind() {
                OperationKind::LoadGlobal(_) => Some(("global", *file, *range)),
                OperationKind::Binary(_) => Some(("binary", *file, *range)),
                OperationKind::LoadProperty(_) => Some(("property", *file, *range)),
                _ => None,
            }
        })
        .collect::<Vec<_>>();

    assert_eq!(ranges.len(), 3);

    let file = ranges[0].1;
    assert_eq!(module.source_file(file).unwrap().name(), "input.mjs");
    assert_eq!(module.source_file(file).unwrap().text(), source);
    assert_eq!(ranges[0], ("global", file, TextRange::new(0, 7)));
    assert_eq!(ranges[1], ("binary", file, TextRange::new(8, 15)));
    assert_eq!(ranges[2], ("property", file, TextRange::new(0, 16)));
}

#[test]
fn records_each_nested_jsx_elements_own_source_range() {
    let source = "const view = <Page.Footer><div>One</div><div><sup>Two</sup></div></Page.Footer>;";
    let module = lower_source_file("input.tsx", source).unwrap();
    let function = module.function(module.entry_function()).unwrap();

    let elements = function
        .operations()
        .filter_map(|(_, operation)| {
            matches!(operation.kind(), OperationKind::JsxElement(_)).then(|| {
                let CompilerLocation::Source { range, .. } =
                    module.location(operation.location()).unwrap()
                else {
                    panic!("JSX operations must have concrete source locations");
                };

                &source[range.start() as usize..range.end() as usize]
            })
        })
        .collect::<Vec<_>>();

    assert_eq!(
        elements,
        [
            "<div>One</div>",
            "<sup>Two</sup>",
            "<div><sup>Two</sup></div>",
            "<Page.Footer><div>One</div><div><sup>Two</sup></div></Page.Footer>",
        ],
    );
}

#[test]
fn gives_every_lowered_operation_source_provenance() {
    let source = "var value; function read() { return value; }";
    let module = lower_source_file("input.mjs", source).unwrap();

    for (_, function) in module.functions() {
        for (_, operation) in function.operations() {
            let CompilerLocation::Source { file, range } =
                module.location(operation.location()).unwrap()
            else {
                panic!("frontend operations must not use unknown provenance");
            };

            assert_eq!(module.source_file(*file).unwrap().text(), source);
            assert!(range.end() <= source.len() as u32);
        }
    }
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
