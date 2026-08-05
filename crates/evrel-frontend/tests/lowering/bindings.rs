//! Binding and assignment lowering.

use super::*;

#[test]
fn lowers_a_const_binding() {
    let module =
        lower_javascript_module(r#"const message = "hello"; console.log(message);"#).unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "region @1 results: 1, parent: region @0, owner: op @5\n",
            "bb1:\n",
            "  %2 = load_binding @0\n",
            "  region_yield %2\n",
            "\n",
            "bb0:\n",
            "  %0 = constant \"hello\"\n",
            "  initialize_binding @0, %0\n",
            "  %1 = load_global \"console\"\n",
            "  %3 = call %1[\"log\"], args: [region @1]",
        )
    );
}

#[test]
fn lowers_a_let_binding_without_an_initializer() {
    let module = lower_javascript_module("let value; console.log(value);").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "region @1 results: 1, parent: region @0, owner: op @5\n",
            "bb1:\n",
            "  %2 = load_binding @0\n",
            "  region_yield %2\n",
            "\n",
            "bb0:\n",
            "  %0 = constant undefined\n",
            "  initialize_binding @0, %0\n",
            "  %1 = load_global \"console\"\n",
            "  %3 = call %1[\"log\"], args: [region @1]",
        )
    );
}

#[test]
fn lowers_a_let_binding_with_an_initializer() {
    let module = lower_javascript_module("let value = 42; console.log(value);").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "region @1 results: 1, parent: region @0, owner: op @5\n",
            "bb1:\n",
            "  %2 = load_binding @0\n",
            "  region_yield %2\n",
            "\n",
            "bb0:\n",
            "  %0 = constant 42\n",
            "  initialize_binding @0, %0\n",
            "  %1 = load_global \"console\"\n",
            "  %3 = call %1[\"log\"], args: [region @1]",
        )
    );
}

#[test]
fn lowers_a_block_scoped_binding() {
    let module = lower_javascript_module("{ let value = 42; consume(value); }").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "region @1 results: 1, parent: region @0, owner: op @5\n",
            "bb1:\n",
            "  %2 = load_binding @0\n",
            "  region_yield %2\n",
            "\n",
            "bb0:\n",
            "  %0 = constant 42\n",
            "  initialize_binding @0, %0\n",
            "  %1 = load_global \"consume\"\n",
            "  %3 = call %1, args: [region @1]",
        )
    );
}

#[test]
fn lowers_a_simple_array_binding_pattern() {
    let module = lower_javascript_module("const [first, , second] = source;").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %0 = load_global \"source\"\n",
            "  %1, %2 = destructure_binding.initialize [@0, _, @1], %0",
        )
    );
}

#[test]
fn lowers_an_array_rest_binding() {
    let module = lower_javascript_module("const [first, ...rest] = source;").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %0 = load_global \"source\"\n",
            "  %1, %2 = destructure_binding.initialize [@0, ...@1], %0",
        )
    );
}

#[test]
fn lowers_a_static_object_binding_pattern() {
    let module =
        lower_javascript_module("const { first, source: renamed, ...rest } = value;").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %0 = load_global \"value\"\n",
            "  %1, %2, %3 = destructure_binding.initialize ",
            "{\"first\": @0, \"source\": @1, ...@2}, %0",
        )
    );
}

#[test]
fn preserves_computed_keys_and_defaults_as_destructuring_regions() {
    let module =
        lower_javascript_module("const { [key()]: value = fallback() } = source;").unwrap();
    let output = print_entry_function(&module);

    assert!(output.contains("region @1 results: 1"));
    assert!(output.contains("load_global \"key\""));
    assert!(output.contains("region @2 results: 1"));
    assert!(output.contains("load_global \"fallback\""));
    assert!(output.contains("{[region @1]: @0 = region @2}"));
    assert!(output.contains("load_global \"source\""));
    assert!(output.contains("destructure_binding.initialize"));
}

#[test]
fn preserves_an_array_binding_default_as_a_region() {
    let module = lower_javascript_module("const [value = fallback()] = source;").unwrap();
    let output = print_entry_function(&module);

    assert!(output.contains("region @1 results: 1"));
    assert!(output.contains("load_global \"fallback\""));
    assert!(output.contains("[@0 = region @1]"));
    assert!(output.contains("destructure_binding.initialize"));
}

#[test]
fn lowers_nested_binding_patterns() {
    let module = lower_javascript_module("const { pair: [first, , ...rest] } = source;").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %0 = load_global \"source\"\n",
            "  %1, %2 = destructure_binding.initialize ",
            "{\"pair\": [@0, _, ...@1]}, %0",
        )
    );
}

#[test]
fn lowers_a_local_assignment() {
    let module = lower_javascript_module("let value = 1; console.log(value = 2, value);").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "region @1 results: 1, parent: region @0, owner: op @8\n",
            "bb1:\n",
            "  %2 = constant 2\n",
            "  store_binding @0, %2\n",
            "  region_yield %2\n",
            "\n",
            "region @2 results: 1, parent: region @0, owner: op @8\n",
            "bb2:\n",
            "  %3 = load_binding @0\n",
            "  region_yield %3\n",
            "\n",
            "bb0:\n",
            "  %0 = constant 1\n",
            "  initialize_binding @0, %0\n",
            "  %1 = load_global \"console\"\n",
            "  %4 = call %1[\"log\"], args: [region @1, region @2]",
        )
    );
}

#[test]
fn lowers_an_array_destructuring_assignment() {
    let module =
        lower_javascript_module("let first; let rest; [first, , ...rest] = source;").unwrap();
    let output = print_entry_function(&module);

    assert!(output.contains("destructure_assignment [@0, _, ...@1]"));
    assert!(output.contains("load_global \"source\""));
}

#[test]
fn preserves_lazy_object_assignment_pattern_expressions() {
    let module =
        lower_javascript_module("({ [key()]: target.value = fallback() } = source);").unwrap();
    let output = print_entry_function(&module);

    assert!(output.contains("destructure_assignment"));
    assert!(output.contains("[region @"));
    assert!(output.contains("property(region @"));
    assert!(output.contains(" = region @"));
    assert!(output.contains("load_global \"source\""));
}

#[test]
fn evaluates_a_computed_assignment_reference_before_the_rhs() {
    let module = lower_javascript_module("getObject()[getKey()] = getValue();").unwrap();

    let output = print_entry_function(&module);

    let object = output.find("load_global \"getObject\"").unwrap();
    let key = output.find("load_global \"getKey\"").unwrap();
    let value = output.find("load_global \"getValue\"").unwrap();
    let store = output.find("store_property").unwrap();

    assert!(object < key);
    assert!(key < value);
    assert!(value < store);
}

#[test]
fn evaluates_a_compound_property_reference_once() {
    let module = lower_javascript_module("getObject()[getKey()] += getValue();").unwrap();

    let output = print_entry_function(&module);

    assert_eq!(
        output,
        concat!(
            "bb0:\n",
            "  %0 = load_global \"getObject\"\n",
            "  %1 = call %0, args: []\n",
            "  %2 = load_global \"getKey\"\n",
            "  %3 = call %2, args: []\n",
            "  %4 = load_property %1, %3\n",
            "  %5 = load_global \"getValue\"\n",
            "  %6 = call %5, args: []\n",
            "  %7 = binary.add %4, %6\n",
            "  store_property %1, %3, %7",
        )
    );

    assert_eq!(output.matches("load_global \"getObject\"").count(), 1);
    assert_eq!(output.matches("load_global \"getKey\"").count(), 1);
    assert_eq!(output.matches("load_property").count(), 1);
    assert_eq!(output.matches("store_property").count(), 1);
}

#[test]
fn lowers_logical_property_assignment_with_a_lazy_rhs() {
    let module = lower_javascript_module("getObject()[getKey()] ||= getValue();").unwrap();

    let output = print_entry_function(&module);

    assert_eq!(output.matches("load_global \"getObject\"").count(), 1);
    assert_eq!(output.matches("load_global \"getKey\"").count(), 1);
    assert_eq!(output.matches("load_property").count(), 1);
    assert_eq!(output.matches("store_property").count(), 1);

    let branch = output.find("if ").unwrap();
    let rhs = output.find("load_global \"getValue\"").unwrap();

    assert!(branch < rhs);
}

#[test]
fn lowers_postfix_update_with_old_and_new_numeric_values() {
    let module = lower_javascript_module("const result = getObject()[getKey()]++;").unwrap();

    let output = print_entry_function(&module);

    assert_eq!(output.matches("load_global \"getObject\"").count(), 1);
    assert_eq!(output.matches("load_global \"getKey\"").count(), 1);
    assert_eq!(output.matches("load_property").count(), 1);
    assert_eq!(output.matches("store_property").count(), 1);

    assert!(output.contains("%5, %6 = update.increment %4"));
    assert!(output.contains("store_property %1, %3, %6"));
    assert!(output.contains("initialize_binding @0, %5"));
}

#[test]
fn lowers_sequence_expressions_left_to_right() {
    let module = lower_javascript_module("const result = (first(), second(), third());").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %0 = load_global \"first\"\n",
            "  %1 = call %0, args: []\n",
            "  %2 = load_global \"second\"\n",
            "  %3 = call %2, args: []\n",
            "  %4 = load_global \"third\"\n",
            "  %5 = call %4, args: []\n",
            "  initialize_binding @0, %5",
        )
    );
}

#[test]
fn lowers_array_elements_in_runtime_order() {
    let module = lower_javascript_module(
        "let assigned; const result = [first(), , ...iterable, assigned = later()];",
    )
    .unwrap();

    let output = print_entry_function(&module);

    assert!(output.contains("region @1 results: 1"));
    assert!(output.contains("region @2 results: 1"));
    assert!(output.contains("region @3 results: 1"));
    assert!(output.contains("load_global \"first\""));
    assert!(output.contains("load_global \"iterable\""));
    assert!(output.contains("load_global \"later\""));
    assert!(output.contains("store_binding @0"));
    assert!(output.contains("array_literal [region @1, _, ...region @2, region @3]"));
}

#[test]
fn lowers_object_properties_in_runtime_order() {
    let module = lower_javascript_module(
        "const result = {
                first: first(),
                [key()]: value(),
                ...source,
                __proto__: prototype
            };",
    )
    .unwrap();

    let output = print_entry_function(&module);

    assert!(output.contains("load_global \"first\""));
    assert!(output.contains("load_global \"key\""));
    assert!(output.contains("load_global \"value\""));
    assert!(output.contains("load_global \"source\""));
    assert!(output.contains("load_global \"prototype\""));
    assert!(output.contains(concat!(
        "object_literal {\"first\": region @1, ",
        "[region @2]: region @3, ...region @4, __proto__: region @5}"
    )));
}

#[test]
fn hoists_and_stores_a_var_binding() {
    let module = lower_javascript_module("console.log(value); var value = 42;").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "region @1 results: 1, parent: region @0, owner: op @5\n",
            "bb1:\n",
            "  %2 = load_binding @0\n",
            "  region_yield %2\n",
            "\n",
            "bb0:\n",
            "  %0 = constant undefined\n",
            "  initialize_binding @0, %0\n",
            "  %1 = load_global \"console\"\n",
            "  %3 = call %1[\"log\"], args: [region @1]\n",
            "  %4 = constant 42\n",
            "  store_binding @0, %4",
        )
    );
}
