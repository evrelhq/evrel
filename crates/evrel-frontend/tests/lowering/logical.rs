//! Conditional and optional value-flow lowering.

use super::*;

#[test]
fn lowers_a_conditional_expression_with_a_join_value() {
    let module = lower_javascript_module("const result = condition ? left() : right();").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %0 = load_global \"condition\"\n",
            "  if %0, then: bb1, else: bb2, completion: bb3\n",
            "\n",
            "bb1:\n",
            "  %2 = load_global \"left\"\n",
            "  %3 = call %2, args: []\n",
            "  jump bb3(%3)\n",
            "\n",
            "bb2:\n",
            "  %4 = load_global \"right\"\n",
            "  %5 = call %4, args: []\n",
            "  jump bb3(%5)\n",
            "\n",
            "bb3(%1):\n",
            "  initialize_binding @0, %1",
        )
    );
}

#[test]
fn lowers_logical_and_with_a_short_circuit_value() {
    let module = lower_javascript_module("const result = left() && right();").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %0 = load_global \"left\"\n",
            "  %1 = call %0, args: []\n",
            "  if %1, then: bb1, else: bb2(%1), completion: bb2\n",
            "\n",
            "bb1:\n",
            "  %3 = load_global \"right\"\n",
            "  %4 = call %3, args: []\n",
            "  jump bb2(%4)\n",
            "\n",
            "bb2(%2):\n",
            "  initialize_binding @0, %2",
        )
    );
}

#[test]
fn lowers_logical_or_with_a_short_circuit_value() {
    let module = lower_javascript_module("const result = left() || right();").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %0 = load_global \"left\"\n",
            "  %1 = call %0, args: []\n",
            "  if %1, then: bb2(%1), else: bb1, completion: bb2\n",
            "\n",
            "bb1:\n",
            "  %3 = load_global \"right\"\n",
            "  %4 = call %3, args: []\n",
            "  jump bb2(%4)\n",
            "\n",
            "bb2(%2):\n",
            "  initialize_binding @0, %2",
        )
    );
}

#[test]
fn lowers_an_optional_static_property_read() {
    let module = lower_javascript_module("object?.property;").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %1 = constant undefined\n",
            "  %2 = load_global \"object\"\n",
            "  %3 = is_nullish %2\n",
            "  if %3, then: bb1(%1), else: bb2, completion: bb1\n",
            "\n",
            "bb1(%0):\n",
            "\n",
            "bb2:\n",
            "  %4 = load_property %2, \"property\"\n",
            "  jump bb1(%4)",
        )
    );
}

#[test]
fn defers_an_optional_computed_property_key() {
    let module = lower_javascript_module("object?.[key];").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %1 = constant undefined\n",
            "  %2 = load_global \"object\"\n",
            "  %3 = is_nullish %2\n",
            "  if %3, then: bb1(%1), else: bb2, completion: bb1\n",
            "\n",
            "bb1(%0):\n",
            "\n",
            "bb2:\n",
            "  %4 = load_global \"key\"\n",
            "  %5 = load_property %2, %4\n",
            "  jump bb1(%5)",
        )
    );
}

#[test]
fn defers_arguments_for_an_optional_method_call() {
    let module = lower_javascript_module("object.method?.(argument);").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "region @1 results: 1, parent: region @0, owner: op @7\n",
            "bb3:\n",
            "  %5 = load_global \"argument\"\n",
            "  region_yield %5\n",
            "\n",
            "bb0:\n",
            "  %1 = constant undefined\n",
            "  %2 = load_global \"object\"\n",
            "  %3 = load_property %2, \"method\"\n",
            "  %4 = is_nullish %3\n",
            "  if %4, then: bb1(%1), else: bb2, completion: bb1\n",
            "\n",
            "bb1(%0):\n",
            "\n",
            "bb2:\n",
            "  %6 = call %3, receiver: %2, args: [region @1]\n",
            "  jump bb1(%6)",
        )
    );
}

#[test]
fn defers_a_method_call_for_an_optional_receiver() {
    let module = lower_javascript_module("object?.method(argument);").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "region @1 results: 1, parent: region @0, owner: op @6\n",
            "bb3:\n",
            "  %4 = load_global \"argument\"\n",
            "  region_yield %4\n",
            "\n",
            "bb0:\n",
            "  %1 = constant undefined\n",
            "  %2 = load_global \"object\"\n",
            "  %3 = is_nullish %2\n",
            "  if %3, then: bb1(%1), else: bb2, completion: bb1\n",
            "\n",
            "bb1(%0):\n",
            "\n",
            "bb2:\n",
            "  %5 = call %2[\"method\"], args: [region @1]\n",
            "  jump bb1(%5)",
        )
    );
}

#[test]
fn short_circuits_the_rest_of_a_continuous_chain() {
    let module = lower_javascript_module("object?.first.second;").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %1 = constant undefined\n",
            "  %2 = load_global \"object\"\n",
            "  %3 = is_nullish %2\n",
            "  if %3, then: bb1(%1), else: bb2, completion: bb1\n",
            "\n",
            "bb1(%0):\n",
            "\n",
            "bb2:\n",
            "  %4 = load_property %2, \"first\"\n",
            "  %5 = load_property %4, \"second\"\n",
            "  jump bb1(%5)",
        )
    );
}

#[test]
fn lowers_an_optional_call_without_a_receiver() {
    let module = lower_javascript_module("functionValue?.(argument);").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "region @1 results: 1, parent: region @0, owner: op @6\n",
            "bb3:\n",
            "  %4 = load_global \"argument\"\n",
            "  region_yield %4\n",
            "\n",
            "bb0:\n",
            "  %1 = constant undefined\n",
            "  %2 = load_global \"functionValue\"\n",
            "  %3 = is_nullish %2\n",
            "  if %3, then: bb1(%1), else: bb2, completion: bb1\n",
            "\n",
            "bb1(%0):\n",
            "\n",
            "bb2:\n",
            "  %5 = call %2, args: [region @1]\n",
            "  jump bb1(%5)",
        )
    );
}

#[test]
fn lowers_logical_not() {
    let module = lower_javascript_module("!value;").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %0 = load_global \"value\"\n",
            "  %1 = unary.not %0",
        )
    );
}

#[test]
fn lowers_value_based_unary_operators() {
    let module = lower_javascript_module("+value; -value; ~value; void value;").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %0 = load_global \"value\"\n",
            "  %1 = unary.plus %0\n",
            "  %2 = load_global \"value\"\n",
            "  %3 = unary.negate %2\n",
            "  %4 = load_global \"value\"\n",
            "  %5 = unary.bitwise_not %4\n",
            "  %6 = load_global \"value\"\n",
            "  %7 = unary.void %6",
        )
    );
}

#[test]
fn distinguishes_value_and_global_typeof() {
    let module = lower_javascript_module("let value = 42; typeof value; typeof missing;").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %0 = constant 42\n",
            "  initialize_binding @0, %0\n",
            "  %1 = load_binding @0\n",
            "  %2 = typeof %1\n",
            "  %3 = typeof_global \"missing\"",
        )
    );
}

#[test]
fn lowers_property_delete_without_reading_the_property() {
    let module = lower_javascript_module("delete object.property; delete object[key];").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %0 = load_global \"object\"\n",
            "  %1 = delete_property %0, \"property\"\n",
            "  %2 = load_global \"object\"\n",
            "  %3 = load_global \"key\"\n",
            "  %4 = delete_property %2, %3",
        )
    );
}

#[test]
fn lowers_delete_of_a_non_reference_value() {
    let module = lower_javascript_module("delete call();").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %0 = load_global \"call\"\n",
            "  %1 = call %0, args: []\n",
            "  %2 = delete_value %1",
        )
    );
}

#[test]
fn lowers_nullish_coalescing_with_a_nullish_test() {
    let module = lower_javascript_module("const result = left() ?? right();").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %0 = load_global \"left\"\n",
            "  %1 = call %0, args: []\n",
            "  %3 = is_nullish %1\n",
            "  if %3, then: bb1, else: bb2(%1), completion: bb2\n",
            "\n",
            "bb1:\n",
            "  %4 = load_global \"right\"\n",
            "  %5 = call %4, args: []\n",
            "  jump bb2(%5)\n",
            "\n",
            "bb2(%2):\n",
            "  initialize_binding @0, %2",
        )
    );
}

#[test]
fn preserves_an_early_return_inside_an_if_statement() {
    let module = lower_javascript_module(
        "const choose = condition => { if (condition) { return 1; } return 2; };",
    )
    .unwrap();
    let arrow = module
        .functions()
        .find_map(|(_, function)| (function.kind() == FunctionKind::Arrow).then_some(function))
        .expect("arrow function must remain live");

    assert_eq!(
        print_function(arrow),
        concat!(
            "param %0 argument @1\n",
            "\n",
            "bb0:\n",
            "  %1 = load_binding @1\n",
            "  if %1, then: bb1, else: bb2, completion: bb2\n",
            "\n",
            "bb1:\n",
            "  %2 = constant 1\n",
            "  return %2\n",
            "\n",
            "bb2:\n",
            "  %3 = constant 2\n",
            "  return %3",
        )
    );
}
