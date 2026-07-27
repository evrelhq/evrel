//! Switch lowering.

use super::*;

#[test]
fn preserves_lazy_switch_selectors_and_default_position() {
    let module = lower_javascript_module(
        "switch (select()) {
                case first():
                    firstBody();
                default:
                    defaultBody();
                case second():
                    secondBody();
            }",
    )
    .unwrap();
    let output = print_module(&module);

    assert_eq!(output.matches("load_global \"select\"").count(), 1);
    assert_eq!(output.matches("case region @").count(), 2);
    assert!(output.contains("default:"));

    let first = output.find("load_global \"first\"").unwrap();
    let second = output.find("load_global \"second\"").unwrap();
    assert!(first < second);
}

#[test]
fn lowers_an_empty_switch_to_its_no_match_target() {
    let module = lower_javascript_module("switch (value) {}").unwrap();
    let output = print_module(&module);

    assert!(output.contains("switch %"));
    assert!(output.contains("cases: []"));
    assert!(output.contains("no_match: bb"));
}

#[test]
fn retains_a_labeled_switch_target_across_a_nested_loop() {
    let module = lower_javascript_module(
        "label: switch (value) {
                case 1:
                    while (test) {
                        break label;
                    }
            }",
    )
    .unwrap();
    let output = print_module(&module);

    assert!(output.contains("labels: [\"label\"]"));
}

#[test]
fn resolves_case_tests_against_switch_wide_lexical_bindings() {
    let module = lower_javascript_module(
        "let outer = 1;
             switch (outer) {
                 case inner:
                     let inner = 2;
             }",
    )
    .unwrap();
    let output = print_module(&module);

    assert!(!output.contains("load_global \"inner\""));
    assert!(output.contains("load_binding @"));
    assert!(output.contains("initialize_binding @"));
}
