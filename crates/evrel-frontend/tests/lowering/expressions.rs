//! Expression lowering.

use super::*;

#[test]
fn lowers_arithmetic_binary_operators() {
    let module = lower_javascript_module("10 - 2; 10 * 2; 10 / 2; 10 % 3; 2 ** 3;").unwrap();

    let output = print_entry_function(&module);

    assert!(output.contains("binary.subtract"));
    assert!(output.contains("binary.multiply"));
    assert!(output.contains("binary.divide"));
    assert!(output.contains("binary.remainder"));
    assert!(output.contains("binary.exponentiate"));
}

#[test]
fn lowers_equality_and_relational_binary_operators() {
    let module = lower_javascript_module(
        "a == b; a != b; a === b; a !== b;\
             a < b; a <= b; a > b; a >= b;\
             a in b; a instanceof b;",
    )
    .unwrap();

    let output = print_entry_function(&module);

    for operator in [
        "loose_equal",
        "loose_not_equal",
        "strict_equal",
        "strict_not_equal",
        "less_than",
        "less_than_or_equal",
        "greater_than",
        "greater_than_or_equal",
        "in",
        "instance_of",
    ] {
        assert!(output.contains(&format!("binary.{operator}")));
    }
}

#[test]
fn lowers_bitwise_and_shift_binary_operators() {
    let module = lower_javascript_module("a << b; a >> b; a >>> b; a | b; a ^ b; a & b;").unwrap();

    let output = print_entry_function(&module);

    for operator in [
        "shift_left",
        "shift_right",
        "unsigned_shift_right",
        "bitwise_or",
        "bitwise_xor",
        "bitwise_and",
    ] {
        assert!(output.contains(&format!("binary.{operator}")));
    }
}

#[test]
fn lowers_a_global_identifier() {
    let module = lower_javascript_module("console;").unwrap();

    assert_eq!(
        print_entry_function(&module),
        "bb0:\n  %0 = load_global \"console\""
    );
}

#[test]
fn lowers_a_static_property_read() {
    let module = lower_javascript_module("console.log;").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %0 = load_global \"console\"\n",
            "  %1 = load_property %0, \"log\"",
        )
    );
}

#[test]
fn lowers_a_computed_property_read() {
    let module = lower_javascript_module("console[20 + 22];").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %0 = load_global \"console\"\n",
            "  %1 = constant 20\n",
            "  %2 = constant 22\n",
            "  %3 = binary.add %1, %2\n",
            "  %4 = load_property %0, %3",
        )
    );
}

#[test]
fn lowers_a_method_call() {
    let module = lower_javascript_module("console.log(20 + 22);").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "region @1 results: 1, parent: region @0, owner: op @5\n",
            "bb1:\n",
            "  %1 = constant 20\n",
            "  %2 = constant 22\n",
            "  %3 = binary.add %1, %2\n",
            "  region_yield %3\n",
            "\n",
            "bb0:\n",
            "  %0 = load_global \"console\"\n",
            "  %4 = call %0[\"log\"], args: [region @1]",
        )
    );
}

#[test]
fn lowers_a_call_without_a_receiver() {
    let module = lower_javascript_module("print(42);").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "region @1 results: 1, parent: region @0, owner: op @3\n",
            "bb1:\n",
            "  %1 = constant 42\n",
            "  region_yield %1\n",
            "\n",
            "bb0:\n",
            "  %0 = load_global \"print\"\n",
            "  %2 = call %0, args: [region @1]",
        )
    );
}

#[test]
fn lowers_a_spread_call_argument() {
    let module = lower_javascript_module("print(...values);").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "region @1 results: 1, parent: region @0, owner: op @3\n",
            "bb1:\n",
            "  %1 = load_global \"values\"\n",
            "  region_yield %1\n",
            "\n",
            "bb0:\n",
            "  %0 = load_global \"print\"\n",
            "  %2 = call %0, args: [...region @1]",
        )
    );
}

#[test]
fn lowers_pure_annotations_on_invocations() {
    let module = lower_javascript_module(
        "plain();
             /* @__PURE__ */ annotated();
             /* #__PURE__ */ new Constructor();
             /* @__PURE__ */ optional?.();
             class Derived extends Base {
                 constructor() {
                     /* #__PURE__ */ super();
                 }
             }",
    )
    .unwrap();

    let mut call_annotations = Vec::new();
    let mut construct_annotations = Vec::new();
    let mut super_call_annotations = Vec::new();

    for (_, function) in module.functions() {
        for (_, operation) in function.operations() {
            match operation.kind() {
                OperationKind::Call(call) => {
                    call_annotations.push(call.has_pure_annotation());
                }
                OperationKind::Construct(construct) => {
                    construct_annotations.push(construct.has_pure_annotation());
                }
                OperationKind::SuperCall(call) => {
                    super_call_annotations.push(call.has_pure_annotation());
                }
                _ => {}
            }
        }
    }

    assert_eq!(
        call_annotations
            .iter()
            .filter(|annotation| **annotation)
            .count(),
        2
    );
    assert!(call_annotations.iter().any(|annotation| !annotation));
    assert_eq!(construct_annotations, [true]);
    assert_eq!(super_call_annotations, [true]);
}

#[test]
fn consumes_a_non_final_spread_before_the_next_argument() {
    let module = lower_javascript_module("print(...values, later());").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "region @1 results: 1, parent: region @0, owner: op @6\n",
            "bb1:\n",
            "  %1 = load_global \"values\"\n",
            "  region_yield %1\n",
            "\n",
            "region @2 results: 1, parent: region @0, owner: op @6\n",
            "bb2:\n",
            "  %2 = load_global \"later\"\n",
            "  %3 = call %2, args: []\n",
            "  region_yield %3\n",
            "\n",
            "bb0:\n",
            "  %0 = load_global \"print\"\n",
            "  %4 = call %0, args: [...region @1, region @2]",
        )
    );
}

#[test]
fn lowers_a_constructor_invocation() {
    let module = lower_javascript_module("new Constructor(42);").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "region @1 results: 1, parent: region @0, owner: op @3\n",
            "bb1:\n",
            "  %1 = constant 42\n",
            "  region_yield %1\n",
            "\n",
            "bb0:\n",
            "  %0 = load_global \"Constructor\"\n",
            "  %2 = construct %0, args: [region @1]",
        )
    );
}

#[test]
fn consumes_a_non_final_constructor_spread_before_the_next_argument() {
    let module = lower_javascript_module("new Constructor(...values, later());").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "region @1 results: 1, parent: region @0, owner: op @6\n",
            "bb1:\n",
            "  %1 = load_global \"values\"\n",
            "  region_yield %1\n",
            "\n",
            "region @2 results: 1, parent: region @0, owner: op @6\n",
            "bb2:\n",
            "  %2 = load_global \"later\"\n",
            "  %3 = call %2, args: []\n",
            "  region_yield %3\n",
            "\n",
            "bb0:\n",
            "  %0 = load_global \"Constructor\"\n",
            "  %4 = construct %0, args: [...region @1, region @2]",
        )
    );
}

#[test]
fn lowers_a_string_argument() {
    let module = lower_javascript_module(r#"console.log("hello");"#).unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "region @1 results: 1, parent: region @0, owner: op @3\n",
            "bb1:\n",
            "  %1 = constant \"hello\"\n",
            "  region_yield %1\n",
            "\n",
            "bb0:\n",
            "  %0 = load_global \"console\"\n",
            "  %2 = call %0[\"log\"], args: [region @1]",
        )
    );
}

#[test]
fn lowers_boolean_and_null_arguments() {
    let module = lower_javascript_module("console.log(true, null);").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "region @1 results: 1, parent: region @0, owner: op @5\n",
            "bb1:\n",
            "  %1 = constant true\n",
            "  region_yield %1\n",
            "\n",
            "region @2 results: 1, parent: region @0, owner: op @5\n",
            "bb2:\n",
            "  %2 = constant null\n",
            "  region_yield %2\n",
            "\n",
            "bb0:\n",
            "  %0 = load_global \"console\"\n",
            "  %3 = call %0[\"log\"], args: [region @1, region @2]",
        )
    );
}
#[test]
fn lowers_numeric_addition() {
    let module = lower_javascript_module("20 + 22;").unwrap();

    assert_eq!(
        print_entry_function(&module),
        "bb0:\n  %0 = constant 20\n  %1 = constant 22\n  %2 = binary.add %0, %1"
    );
}
