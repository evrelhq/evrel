//! Object and property lowering.

use super::*;

#[test]
fn lowers_private_name_membership_checks() {
    let module = lower_javascript_module(
        "class Example {
                #value;

                static has(object) {
                    return #value in object;
                }
            }",
    )
    .unwrap();
    let output = print_module(&module);

    assert_eq!(module.private_name_count(), 1);
    assert!(output.contains("has_private_name"));
    assert!(output.contains("private @0"));
}

#[test]
fn lowers_optional_private_reads_and_calls() {
    let module = lower_javascript_module(
        "class Example {
                #value;
                #method() {}

                read(object) {
                    object?.#value;
                    object?.#method();
                    object.#method?.();
                }
            }",
    )
    .unwrap();
    let output = print_module(&module);

    assert!(output.contains("is_nullish"));
    assert!(output.contains("private @0"));
    assert!(output.contains("private @1"));
    assert!(output.contains("receiver:"));
}

#[test]
fn lowers_object_literal_methods_and_accessors() {
    let module = lower_javascript_module(
        "const object = {
                method(value) { return value; },
                get value() { return 1; },
                set value(next) {},
                async *[key()]() { yield 1; }
            };",
    )
    .unwrap();
    let output = print_module(&module);
    let method_count = module
        .functions()
        .filter(|(_, function)| function.kind() == FunctionKind::ObjectMethod)
        .count();

    assert_eq!(method_count, 4);
    assert!(output.contains("method \"method\": function"));
    assert!(output.contains("get \"value\": function"));
    assert!(output.contains("set \"value\": function"));
    assert!(output.contains("method [region @"));
    assert!(output.contains("object_method async_generator"));
}

#[test]
fn lowers_debugger_as_an_observable_operation() {
    let module = lower_javascript_module("debugger;").unwrap();
    let function = module.function(module.entry_function()).unwrap();
    let operation = function
        .blocks()
        .flat_map(|(_, block)| block.operations())
        .map(|operation| function.operation(*operation).unwrap())
        .find(|operation| matches!(operation.kind(), OperationKind::Debugger(_)))
        .unwrap();

    assert!(
        operation
            .kind()
            .intrinsic_effects()
            .may_have_observable_effects()
    );
    assert_eq!(print_function(function), "bb0:\n  debugger");
}

#[test]
fn lowers_static_and_computed_super_property_reads() {
    let module = lower_javascript_module(
        "class Derived extends Base {
                method() {
                    return super.value + super[key()];
                }
            }",
    )
    .unwrap();
    let output = print_module(&module);

    assert!(output.contains("load_super_property \"value\""));
    assert!(output.contains("load_super_property %"));
}

#[test]
fn lowers_super_property_assignments_and_updates() {
    let module = lower_javascript_module(
        "class Derived extends Base {
                method() {
                    super.value = 1;
                    super.value += 2;
                    super[key()] ||= fallback();
                    super.count++;
                }
            }",
    )
    .unwrap();
    let output = print_module(&module);

    assert!(output.contains("store_super_property \"value\""));
    assert!(output.contains("load_super_property \"value\""));
    assert!(output.contains("store_super_property %"));
    assert_eq!(output.matches("load_global \"key\"").count(), 1);
    assert!(output.contains("update.increment"));
}

#[test]
fn lowers_static_and_computed_super_method_calls() {
    let module = lower_javascript_module(
        "class Derived extends Base {
                method() {
                    super.run(1);
                    super[key()](2);
                }
            }",
    )
    .unwrap();
    let output = print_module(&module);

    assert!(output.contains("call super.\"run\", args:"));
    assert!(output.contains("call super[%"));
    assert_eq!(output.matches("load_global \"key\"").count(), 1);
    assert!(!output.contains("load_super_property \"run\""));
}

#[test]
fn lowers_super_constructor_calls() {
    let module = lower_javascript_module(
        "class Derived extends Base {
                constructor(first, values) {
                    super(first, ...values, last());
                }
            }",
    )
    .unwrap();
    let output = print_module(&module);

    assert!(output.contains("super_call args: [region @"));
    assert!(output.contains("...region @"));
    assert!(!output.contains("load_global \"super\""));
}

#[test]
fn lowers_tagged_templates_with_receiver_and_site_identity() {
    let module = lower_javascript_module(
        "const object = {
                tag(strings, value) { return value; }
            };
            object.tag`value: ${compute()}`;",
    )
    .unwrap();
    let output = print_module(&module);

    assert_eq!(module.template_site_count(), 1);
    assert!(output.contains("tagged_template site @0"));
    assert!(output.contains("target: %"));
    assert!(output.contains("[\"tag\"]"));
    assert!(output.contains("substitutions: [region @"));
    assert!(output.contains("load_global \"compute\""));
}

#[test]
fn preserves_invalid_tagged_template_escapes_as_uncooked() {
    let module = lower_javascript_module("tag`\\unicode`;").unwrap();
    let output = print_module(&module);

    assert!(output.contains("raw: \"\\\\unicode\""));
    assert!(output.contains("cooked: undefined"));
}

#[test]
fn lowers_unresolved_global_assignments() {
    let module = lower_javascript_module(
        "globalValue = create();
             globalValue += 2;
             globalValue ||= fallback();
             globalValue++;",
    )
    .unwrap();
    let output = print_module(&module);

    assert_eq!(output.matches("store_global \"globalValue\"").count(), 4);
    assert_eq!(output.matches("load_global \"globalValue\"").count(), 3);
}
