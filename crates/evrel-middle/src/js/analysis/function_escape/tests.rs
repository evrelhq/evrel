use evrel_frontend::lower_source_file;
use evrel_js_ir::{FunctionId, JsModuleIr, OperationKind, ValueId};

use super::{FunctionEscapeAnalysis, ValueEscape};
use crate::js::transform::promote_bindings_to_ssa;

#[test]
fn classifies_return_call_and_unused_values() {
    let module = lower_source_file(
        "entry.js",
        r#"
            function test(consume) {
                const unused = {};
                const passed = {};
                consume(passed);
                return {};
            }
        "#,
    )
    .unwrap();
    let function = test_function(&module);
    let analysis = FunctionEscapeAnalysis::analyze(&module, function).unwrap();
    let objects = operation_results(&module, function, |kind| {
        matches!(kind, OperationKind::ObjectLiteral(_))
    });

    assert_eq!(objects.len(), 3);
    assert_eq!(
        analysis.escape_result(objects[0]),
        Some(ValueEscape::DoesNotEscape)
    );
    assert_eq!(
        analysis.escape_result(objects[1]),
        Some(ValueEscape::MayEscape)
    );
    assert_eq!(
        analysis.escape_result(objects[2]),
        Some(ValueEscape::MayEscape)
    );
}

#[test]
fn treats_unknown_origins_conservatively() {
    let module = lower_source_file(
        "entry.js",
        "function test(create) { const value = create(); return value === null; }",
    )
    .unwrap();
    let function = test_function(&module);
    let call = operation_result(&module, function, |kind| {
        matches!(kind, OperationKind::Call(_))
    });
    let analysis = FunctionEscapeAnalysis::analyze(&module, function).unwrap();

    assert_eq!(analysis.escape_result(call), None);
    assert!(analysis.may_escape(call));
}

#[test]
fn follows_local_bindings_and_forwarded_block_values() {
    let mut module = lower_source_file(
        "entry.js",
        r#"
            function test(condition) {
                const value = {};
                var selected;
                if (condition) selected = value;
                else selected = value;
                return selected;
            }
        "#,
    )
    .unwrap();
    let function = test_function(&module);
    assert_eq!(promote_bindings_to_ssa(&mut module), 1);
    let object = operation_result(&module, function, |kind| {
        matches!(kind, OperationKind::ObjectLiteral(_))
    });

    assert!(
        FunctionEscapeAnalysis::analyze(&module, function)
            .unwrap()
            .may_escape(object)
    );
}

#[test]
fn escaping_containers_escape_their_contents() {
    let module = lower_source_file(
        "entry.js",
        r#"
            function test() {
                const child = {};
                return { child };
            }
        "#,
    )
    .unwrap();
    let function = test_function(&module);
    let objects = operation_results(&module, function, |kind| {
        matches!(kind, OperationKind::ObjectLiteral(_))
    });
    let analysis = FunctionEscapeAnalysis::analyze(&module, function).unwrap();

    assert_eq!(objects.len(), 2);
    assert!(objects.into_iter().all(|value| analysis.may_escape(value)));
}

#[test]
fn follows_values_captured_by_an_escaping_closure() {
    let module = lower_source_file(
        "entry.js",
        r#"
            function test() {
                const captured = {};
                return () => captured;
            }
        "#,
    )
    .unwrap();
    let function = test_function(&module);
    let object = operation_result(&module, function, |kind| {
        matches!(kind, OperationKind::ObjectLiteral(_))
    });

    assert!(
        FunctionEscapeAnalysis::analyze(&module, function)
            .unwrap()
            .may_escape(object)
    );
}

#[test]
fn strict_comparison_observes_without_escaping() {
    let module = lower_source_file(
        "entry.js",
        r#"
            function test() {
                const value = {};
                return value === null;
            }
        "#,
    )
    .unwrap();
    let function = test_function(&module);
    let object = operation_result(&module, function, |kind| {
        matches!(kind, OperationKind::ObjectLiteral(_))
    });

    assert_eq!(
        FunctionEscapeAnalysis::analyze(&module, function)
            .unwrap()
            .escape_result(object),
        Some(ValueEscape::DoesNotEscape)
    );
}

#[test]
fn exported_bindings_escape_their_values() {
    let module = lower_source_file("entry.js", "export const value = {};").unwrap();
    let function = module.entry_function();
    let object = operation_result(&module, function, |kind| {
        matches!(kind, OperationKind::ObjectLiteral(_))
    });

    assert_eq!(
        FunctionEscapeAnalysis::analyze(&module, function)
            .unwrap()
            .escape_result(object),
        Some(ValueEscape::MayEscape)
    );
}

#[test]
fn dynamic_import_promises_escape_to_the_host() {
    let module =
        lower_source_file("entry.js", "function test() { import('./dependency.js'); }").unwrap();
    let function = test_function(&module);
    let promise = operation_result(&module, function, |kind| {
        matches!(kind, OperationKind::DynamicImport(_))
    });

    assert_eq!(
        FunctionEscapeAnalysis::analyze(&module, function)
            .unwrap()
            .escape_result(promise),
        Some(ValueEscape::MayEscape)
    );
}

fn test_function(module: &JsModuleIr) -> FunctionId {
    module
        .functions()
        .find_map(|(function, data)| {
            (data.parent_function() == Some(module.entry_function())).then_some(function)
        })
        .expect("test function must exist")
}

fn operation_result(
    module: &JsModuleIr,
    function: FunctionId,
    predicate: impl FnMut(&OperationKind) -> bool,
) -> ValueId {
    operation_results(module, function, predicate)
        .into_iter()
        .next()
        .expect("matching operation must exist")
}

fn operation_results(
    module: &JsModuleIr,
    function: FunctionId,
    mut predicate: impl FnMut(&OperationKind) -> bool,
) -> Vec<ValueId> {
    module
        .function(function)
        .expect("function must exist")
        .operations()
        .filter(|(_, operation)| predicate(operation.kind()))
        .map(|(_, operation)| operation.results()[0])
        .collect()
}
