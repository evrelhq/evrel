//! Class lowering.

use super::*;

#[test]
fn lowers_an_empty_class_expression() {
    let module = lower_javascript_module("(class {});").unwrap();

    assert_eq!(
        print_entry_function(&module),
        "bb0:\n  %0 = create_class self: none, super: none, elements: []"
    );
}

#[test]
fn lowers_class_heritage_as_an_expression_region() {
    let module = lower_javascript_module("(class extends getBase() {});").unwrap();
    let output = print_entry_function(&module);

    assert!(output.contains("load_global \"getBase\""));
    assert!(output.contains("call"));
    assert!(output.contains("create_class self: none, super: region @1, elements: []"));
}

#[test]
fn lowers_a_named_class_expression_self_binding() {
    let module = lower_javascript_module("(class Internal {});").unwrap();
    let output = print_module(&module);

    assert!(output.contains("binding @0 class \"Internal\""));
    assert!(output.contains("create_class self: @0, super: none, elements: []"));
}

#[test]
fn lowers_an_empty_class_declaration() {
    let module = lower_javascript_module("class Example {}").unwrap();
    let output = print_module(&module);

    assert!(output.contains("binding @0 class \"Example\""));
    assert!(output.contains("create_class self: @0, super: none, elements: []"));
    assert!(output.contains("initialize_binding @0, %0"));
}

#[test]
fn lowers_an_exported_class_declaration() {
    let module = lower_javascript_module("export class Example {}").unwrap();
    let output = print_module(&module);

    assert!(output.contains("export @0 as Example"));
    assert!(output.contains("binding @0 class \"Example\""));
    assert!(output.contains("create_class self: @0, super: none, elements: []"));
    assert!(output.contains("initialize_binding @0, %0"));
}

#[test]
fn lowers_a_named_default_class() {
    let module = lower_javascript_module("export default class Example {}").unwrap();
    let output = print_module(&module);

    assert!(output.contains("export @0 as default"));
    assert!(output.contains("binding @0 class \"Example\""));
    assert!(output.contains("create_class self: @0"));
}

#[test]
fn lowers_an_anonymous_default_class() {
    let module = lower_javascript_module("export default class {}").unwrap();
    let output = print_module(&module);

    assert!(output.contains("export @0 as default"));
    assert!(output.contains("binding @0 class \"*default*\""));
    assert!(output.contains("create_class self: none"));
    assert!(output.contains("initialize_binding @0, %0"));
}

#[test]
fn lowers_prototype_and_static_class_methods() {
    let module = lower_javascript_module(
        "class Example {
                answer() { return 42; }
                static create() { return new Example(); }
            }",
    )
    .unwrap();
    let output = print_module(&module);

    assert!(output.contains("prototype method \"answer\": @1"));
    assert!(output.contains("static method \"create\": @2"));
    assert!(output.contains("function @1 class_method"));
    assert!(output.contains("function @2 class_method"));
}

#[test]
fn lowers_class_getters_and_setters() {
    let module = lower_javascript_module(
        "class Example {
                get value() { return 42; }
                set value(next) { this.current = next; }
                static get total() { return 1; }
            }",
    )
    .unwrap();
    let output = print_module(&module);

    assert!(output.contains("prototype getter \"value\": @1"));
    assert!(output.contains("prototype setter \"value\": @2"));
    assert!(output.contains("static getter \"total\": @3"));
}

#[test]
fn lowers_a_class_constructor() {
    let module = lower_javascript_module(
        "class Example {
                constructor(value) {
                    this.value = value;
                }
            }",
    )
    .unwrap();
    let output = print_module(&module);

    assert!(output.contains("prototype constructor \"constructor\": @1"));
    assert!(output.contains("function @1 class_constructor"));
    assert!(output.contains("store_property"));
}

#[test]
fn rejects_typescript_parameter_properties() {
    let result = lower_typescript_module(
        "
            class Item {
                constructor(public readonly name: string) {}
            }
            ",
    );
    assert!(matches!(
        result,
        Err(FrontendError::UnsupportedParameterProperty)
    ));
}

#[test]
fn lowers_computed_class_method_keys_in_source_order() {
    let module = lower_javascript_module(
        "class Example {
                [first()]() {}
                static [second()]() {}
            }",
    )
    .unwrap();
    let output = print_module(&module);

    assert!(output.contains("prototype method [region @1]: @1"));
    assert!(output.contains("static method [region @2]: @2"));
    assert!(output.contains("load_global \"first\""));
    assert!(output.contains("load_global \"second\""));
}

#[test]
fn lowers_static_literal_class_method_keys() {
    let module = lower_javascript_module(
        r#"class Example {
                "quoted"() {}
                1e3() {}
            }"#,
    )
    .unwrap();
    let output = print_module(&module);

    assert!(output.contains("prototype method \"quoted\": @1"));
    assert!(output.contains("prototype method \"1000\": @2"));
}

#[test]
fn lowers_instance_and_static_class_fields() {
    let module = lower_javascript_module(
        "class Example {
                value = this.compute();
                empty;
                static count = initialize();
            }",
    )
    .unwrap();
    let output = print_module(&module);

    assert!(output.contains("instance field \"value\": @1"));
    assert!(output.contains("instance field \"empty\": none"));
    assert!(output.contains("static field \"count\": @2"));
    assert_eq!(output.matches("class_field_initializer").count(), 2);
}

#[test]
fn lowers_class_static_blocks_as_non_callable_function_bodies() {
    let module = lower_javascript_module(
        "class Example {
                static {
                    var local = initialize();
                    if (local) this.value = local;
                }
                static count = 1;
            }",
    )
    .unwrap();
    let output = print_module(&module);

    assert!(output.contains("static block: @1"));
    assert!(output.contains("static field \"count\": @2"));
    assert!(output.contains("function @1 class_static_block"));
    assert!(output.contains("binding @1 var \"local\""));
    assert_eq!(
        module
            .functions()
            .filter(|(_, function)| function.kind() == FunctionKind::ClassStaticBlock)
            .count(),
        1
    );
}

#[test]
fn lowers_private_class_element_names() {
    let module = lower_javascript_module(
        "class Example {
                #field;
                get #value() {}
                set #value(next) {}
            }",
    )
    .unwrap();
    let output = print_module(&module);

    assert_eq!(module.private_name_count(), 2);
    assert!(output.contains("private_name @0 \"field\""));
    assert!(output.contains("private_name @1 \"value\""));
    assert_eq!(output.matches("private @1").count(), 2);
}

#[test]
fn lowers_private_property_reads_and_method_calls() {
    let module = lower_javascript_module(
        "class Example {
                #value = 42;

                #read() {
                    return this.#value;
                }

                read() {
                    return this.#read();
                }
            }",
    )
    .unwrap();
    let output = print_module(&module);

    assert!(output.contains("load_property %0, private @0"));
    assert!(output.contains("call %0[private @1]"));
}

#[test]
fn lowers_private_assignments_and_updates() {
    let module = lower_javascript_module(
        "class Example {
                #value = 0;

                update(next) {
                    target().#value += next;
                    return this.#value++;
                }
            }",
    )
    .unwrap();
    let output = print_module(&module);

    assert_eq!(output.matches("load_global \"target\"").count(), 1);
    assert_eq!(output.matches("load_property").count(), 2);
    assert_eq!(output.matches("store_property").count(), 2);
    assert!(output.contains("private @0"));
}
