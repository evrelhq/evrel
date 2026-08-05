use evrel_frontend::lower_source_file;
use evrel_js_ir::{FunctionId, JsModuleIr, OperationKind, ValueId};

use super::{AliasResult, EscapeResult, FunctionPointerAnalysis};
use crate::js::transform::promote_bindings_to_ssa;

#[test]
fn distinguishes_fresh_allocations() {
    let module = lower_source_file(
        "entry.js",
        r#"
            function test() {
                const first = {};
                const second = {};
                return first === second;
            }
        "#,
    )
    .unwrap();
    let function = test_function(&module);
    let objects = operation_results(&module, function, |kind| {
        matches!(kind, OperationKind::ObjectLiteral(_))
    });
    let analysis = FunctionPointerAnalysis::analyze(&module, function).unwrap();

    assert_eq!(objects.len(), 2);
    assert_eq!(analysis.alias(objects[0], objects[1]), AliasResult::NoAlias);
    assert_eq!(
        analysis.alias(objects[0], objects[0]),
        AliasResult::MustAlias
    );
}

#[test]
fn treats_unknown_values_as_possible_aliases() {
    let module = lower_source_file(
        "entry.js",
        "function test(left, right) { return left === right; }",
    )
    .unwrap();
    let function = test_function(&module);
    let function_ir = module.function(function).unwrap();
    let left = function_ir.parameters()[0].value();
    let right = function_ir.parameters()[1].value();
    let analysis = FunctionPointerAnalysis::analyze(&module, function).unwrap();

    assert_eq!(analysis.alias(left, right), AliasResult::MayAlias);
}

#[test]
fn recognizes_stable_arguments_object_identity() {
    let module = lower_source_file(
        "entry.js",
        "function test() { return arguments === arguments; }",
    )
    .unwrap();
    let function = test_function(&module);
    let arguments = operation_results(&module, function, |kind| {
        matches!(kind, OperationKind::LoadArguments(_))
    });
    let analysis = FunctionPointerAnalysis::analyze(&module, function).unwrap();

    assert_eq!(arguments.len(), 2);
    assert_eq!(
        analysis.alias(arguments[0], arguments[1]),
        AliasResult::MustAlias
    );
}

#[test]
fn named_function_self_binding_is_an_object_boundary() {
    let module = lower_source_file(
        "entry.js",
        "const value = function named() { return named; };",
    )
    .unwrap();
    let function = test_function(&module);
    let self_load = operation_result(&module, function, |kind| {
        matches!(kind, OperationKind::LoadBinding(_))
    });
    let analysis = FunctionPointerAnalysis::analyze(&module, function).unwrap();

    assert_eq!(analysis.alias(self_load, self_load), AliasResult::MustAlias);
    assert!(analysis.may_escape(self_load));
}

#[test]
fn primitive_values_do_not_alias_objects() {
    let module = lower_source_file(
        "entry.js",
        "function test() { const value = {}; return value === 1; }",
    )
    .unwrap();
    let function = test_function(&module);
    let object = operation_result(&module, function, |kind| {
        matches!(kind, OperationKind::ObjectLiteral(_))
    });
    let number = operation_result(&module, function, |kind| {
        matches!(kind, OperationKind::Constant(_))
    });
    let analysis = FunctionPointerAnalysis::analyze(&module, function).unwrap();

    assert_eq!(analysis.alias(object, number), AliasResult::NoAlias);
    assert_eq!(analysis.alias(number, number), AliasResult::NoAlias);
}

#[test]
fn construction_results_are_definitely_objects() {
    let module = lower_source_file(
        "entry.js",
        "function test(Constructor) { return new Constructor(); }",
    )
    .unwrap();
    let function = test_function(&module);
    let constructed = operation_result(&module, function, |kind| {
        matches!(kind, OperationKind::Construct(_))
    });
    let analysis = FunctionPointerAnalysis::analyze(&module, function).unwrap();

    assert_eq!(
        analysis.alias(constructed, constructed),
        AliasResult::MustAlias
    );
    assert!(analysis.may_escape(constructed));
}

#[test]
fn propagates_allocations_through_local_binding_copies() {
    let module = lower_source_file(
        "entry.js",
        r#"
            function test() {
                const original = {};
                const copy = original;
                return copy;
            }
        "#,
    )
    .unwrap();
    let function = test_function(&module);
    let object = operation_result(&module, function, |kind| {
        matches!(kind, OperationKind::ObjectLiteral(_))
    });
    let copies = operation_results(&module, function, |kind| {
        matches!(kind, OperationKind::LoadBinding(_))
    });
    let analysis = FunctionPointerAnalysis::analyze(&module, function).unwrap();

    assert!(!copies.is_empty());
    assert_eq!(
        analysis.alias(object, *copies.last().unwrap()),
        AliasResult::MayAlias
    );
    assert_eq!(
        analysis.escape_result(object),
        Some(EscapeResult::MayEscape)
    );
}

#[test]
fn destructured_values_remain_conservatively_unknown() {
    let module = lower_source_file(
        "entry.js",
        r#"
            function test(source) {
                const { value } = source;
                return value;
            }
        "#,
    )
    .unwrap();
    let function = test_function(&module);
    let value = operation_result(&module, function, |kind| {
        matches!(kind, OperationKind::LoadBinding(_))
    });
    let analysis = FunctionPointerAnalysis::analyze(&module, function).unwrap();

    assert!(
        analysis
            .points_to(value)
            .unwrap()
            .may_point_to_unknown_object()
    );
    assert!(analysis.may_escape(value));
}

#[test]
fn classifies_return_call_and_unused_allocations() {
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
    let analysis = FunctionPointerAnalysis::analyze(&module, function).unwrap();
    let objects = operation_results(&module, function, |kind| {
        matches!(kind, OperationKind::ObjectLiteral(_))
    });

    assert_eq!(objects.len(), 3);
    assert_eq!(
        analysis.escape_result(objects[0]),
        Some(EscapeResult::DoesNotEscape)
    );
    assert_eq!(
        analysis.escape_result(objects[1]),
        Some(EscapeResult::MayEscape)
    );
    assert_eq!(
        analysis.escape_result(objects[2]),
        Some(EscapeResult::MayEscape)
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
    let analysis = FunctionPointerAnalysis::analyze(&module, function).unwrap();

    assert_eq!(analysis.escape_result(call), None);
    assert_eq!(analysis.alias(call, call), AliasResult::MayAlias);
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
        FunctionPointerAnalysis::analyze(&module, function)
            .unwrap()
            .may_escape(object)
    );
}

#[test]
fn preserves_local_objects_when_merged_with_unknown_values() {
    let mut module = lower_source_file(
        "entry.js",
        r#"
            function test(condition, external) {
                const local = {};
                var selected;
                if (condition) selected = local;
                else selected = external;
                return selected;
            }
        "#,
    )
    .unwrap();
    let function = test_function(&module);
    assert_eq!(promote_bindings_to_ssa(&mut module), 1);
    let local = operation_result(&module, function, |kind| {
        matches!(kind, OperationKind::ObjectLiteral(_))
    });
    let analysis = FunctionPointerAnalysis::analyze(&module, function).unwrap();

    assert_eq!(analysis.escape_result(local), Some(EscapeResult::MayEscape));
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
    let analysis = FunctionPointerAnalysis::analyze(&module, function).unwrap();

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
        FunctionPointerAnalysis::analyze(&module, function)
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
        FunctionPointerAnalysis::analyze(&module, function)
            .unwrap()
            .escape_result(object),
        Some(EscapeResult::DoesNotEscape)
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
        FunctionPointerAnalysis::analyze(&module, function)
            .unwrap()
            .escape_result(object),
        Some(EscapeResult::MayEscape)
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
        FunctionPointerAnalysis::analyze(&module, function)
            .unwrap()
            .escape_result(promise),
        Some(EscapeResult::MayEscape)
    );
}

#[test]
fn values_retained_by_async_continuations_escape() {
    let module = lower_source_file(
        "entry.js",
        r#"
            async function test() {
                const value = {};
                await 0;
                return value === value;
            }
        "#,
    )
    .unwrap();
    let function = test_function(&module);
    let object = operation_result(&module, function, |kind| {
        matches!(kind, OperationKind::ObjectLiteral(_))
    });
    let analysis = FunctionPointerAnalysis::analyze(&module, function).unwrap();

    assert_eq!(
        analysis.escape_result(object),
        Some(EscapeResult::MayEscape)
    );
}

#[test]
fn values_retained_by_generator_continuations_escape() {
    let module = lower_source_file(
        "entry.js",
        r#"
            function* test() {
                const value = {};
                yield 0;
                return value === value;
            }
        "#,
    )
    .unwrap();
    let function = test_function(&module);
    let object = operation_result(&module, function, |kind| {
        matches!(kind, OperationKind::ObjectLiteral(_))
    });
    let analysis = FunctionPointerAnalysis::analyze(&module, function).unwrap();

    assert_eq!(
        analysis.escape_result(object),
        Some(EscapeResult::MayEscape)
    );
}

#[test]
fn rejects_objects_from_another_function_analysis() {
    let module = lower_source_file(
        "entry.js",
        r#"
            function first() { return arguments; }
            function second() { return arguments; }
        "#,
    )
    .unwrap();
    let functions = module
        .functions()
        .filter_map(|(function, data)| {
            (data.parent_function() == Some(module.entry_function())).then_some(function)
        })
        .collect::<Vec<_>>();
    let first_arguments = operation_result(&module, functions[0], |kind| {
        matches!(kind, OperationKind::LoadArguments(_))
    });
    let first_analysis = FunctionPointerAnalysis::analyze(&module, functions[0]).unwrap();
    let second_analysis = FunctionPointerAnalysis::analyze(&module, functions[1]).unwrap();
    let object = first_analysis
        .points_to(first_arguments)
        .unwrap()
        .objects()
        .next()
        .unwrap();

    assert_eq!(object.function(), functions[0]);
    assert_eq!(second_analysis.object_escape_result(object), None);
    assert!(second_analysis.object_may_escape(object));
}

#[test]
fn rejects_objects_from_another_snapshot_of_the_same_function() {
    let module = lower_source_file("entry.js", "function test() { return arguments; }").unwrap();
    let function = test_function(&module);
    let arguments = operation_result(&module, function, |kind| {
        matches!(kind, OperationKind::LoadArguments(_))
    });
    let first_analysis = FunctionPointerAnalysis::analyze(&module, function).unwrap();
    let second_analysis = FunctionPointerAnalysis::analyze(&module, function).unwrap();
    let object = first_analysis
        .points_to(arguments)
        .unwrap()
        .objects()
        .next()
        .unwrap();

    assert!(first_analysis.object(object).is_some());
    assert_eq!(second_analysis.object_escape_result(object), None);
    assert!(second_analysis.object_may_escape(object));
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
