//! Function lowering.

use super::*;

#[test]
fn lowers_a_named_function_expression_self_binding() {
    let module =
        lower_javascript_module("const read = function recurse() { return recurse; };").unwrap();

    let function = module
        .functions()
        .find_map(|(_, function)| (function.kind() == FunctionKind::Ordinary).then_some(function))
        .unwrap();
    let binding = function.self_binding().unwrap();
    let output = print_function(function);

    assert_eq!(module.binding(binding).unwrap().name(), "recurse");
    assert!(output.contains("self_binding @1"));
    assert!(output.contains("load_binding @1"));
    assert!(!output.contains("initialize_binding @1"));
}

#[test]
fn instantiates_a_function_declaration_before_statements() {
    let module = lower_javascript_module("read(); function read() { return 42; }").unwrap();
    let output = print_entry_function(&module);

    let create = output.find("create_function @1").unwrap();
    let initialize = output.find("initialize_binding @0").unwrap();
    let load = output.find("load_binding @0").unwrap();

    assert!(create < initialize);
    assert!(initialize < load);
}

#[test]
fn instantiates_a_nested_function_declaration() {
    let module =
        lower_javascript_module("function outer() { inner(); function inner() { return 42; } }")
            .unwrap();
    let functions = module.functions().collect::<Vec<_>>();

    assert_eq!(functions.len(), 3);
    assert!(print_function(functions[1].1).contains("create_function @2"));
}

#[test]
fn stores_a_function_declaration_over_a_parameter_binding() {
    let module = lower_javascript_module(
        "function outer(value) { function value() { return 42; } return value; }",
    )
    .unwrap();
    let functions = module.functions().collect::<Vec<_>>();

    assert!(print_function(functions[1].1).contains("store_binding @1"));
}

#[test]
fn instantiates_only_the_last_same_name_function_declaration() {
    let module = lower_javascript_module(
        "function outer() {
                function selected() { return 1; }
                function selected() { return 2; }
            }",
    )
    .unwrap();

    assert_eq!(module.functions().count(), 3);
}

#[test]
fn instantiates_a_block_scoped_function_on_block_entry() {
    let module = lower_javascript_module(
        "if (enabled) {
                read();
                function read() { return 42; }
            }",
    )
    .unwrap();
    let output = print_entry_function(&module);

    let block = output.find("bb1:").unwrap();
    let create = output.find("create_function @1").unwrap();
    let initialize = output.find("initialize_binding @0").unwrap();
    let load = output.find("load_binding @0").unwrap();

    assert!(block < create);
    assert!(create < initialize);
    assert!(initialize < load);
}

#[test]
fn instantiates_function_declarations_inside_try_and_catch_blocks() {
    let module = lower_javascript_module(
        "try {
                insideTry();
                function insideTry() {}
            } catch {
                insideCatch();
                function insideCatch() {}
            }",
    )
    .unwrap();

    assert_eq!(module.functions().count(), 3);

    let output = print_entry_function(&module);

    assert!(output.contains("create_function @1"));
    assert!(output.contains("create_function @2"));
}

#[test]
fn lowers_this_in_an_ordinary_function() {
    let module = lower_javascript_module("const read = function () { return this; };").unwrap();

    assert_eq!(
        print_module(&module),
        concat!(
            "module {\n",
            "  binding @0 const \"read\"\n",
            "\n",
            "  function @0 entry {\n",
            "    bb0:\n",
            "      %0 = create_function @1\n",
            "      initialize_binding @0, %0\n",
            "  }\n",
            "\n",
            "  function @1 ordinary {\n",
            "    bb0:\n",
            "      %0 = load_this\n",
            "      return %0\n",
            "  }\n",
            "}",
        )
    );
}

#[test]
fn lowers_lexical_this_in_a_nested_arrow() {
    let module =
        lower_javascript_module("const outer = function () { return () => this; };").unwrap();

    assert_eq!(
        print_module(&module),
        concat!(
            "module {\n",
            "  binding @0 const \"outer\"\n",
            "\n",
            "  function @0 entry {\n",
            "    bb0:\n",
            "      %0 = create_function @1\n",
            "      initialize_binding @0, %0\n",
            "  }\n",
            "\n",
            "  function @1 ordinary {\n",
            "    bb0:\n",
            "      %0 = create_function @2\n",
            "      return %0\n",
            "  }\n",
            "\n",
            "  function @2 arrow {\n",
            "    bb0:\n",
            "      %0 = load_this\n",
            "      return %0\n",
            "  }\n",
            "}",
        )
    );
}

#[test]
fn lowers_lexical_arguments_in_a_nested_arrow() {
    let module =
        lower_javascript_module("const outer = function () { return () => arguments; };").unwrap();

    assert_eq!(
        print_module(&module),
        concat!(
            "module {\n",
            "  binding @0 const \"outer\"\n",
            "\n",
            "  function @0 entry {\n",
            "    bb0:\n",
            "      %0 = create_function @1\n",
            "      initialize_binding @0, %0\n",
            "  }\n",
            "\n",
            "  function @1 ordinary {\n",
            "    bb0:\n",
            "      %0 = create_function @2\n",
            "      return %0\n",
            "  }\n",
            "\n",
            "  function @2 arrow {\n",
            "    bb0:\n",
            "      %0 = load_arguments\n",
            "      return %0\n",
            "  }\n",
            "}",
        )
    );
}

#[test]
fn keeps_module_arguments_as_a_global() {
    let module = lower_javascript_module("const read = () => arguments;").unwrap();
    let arrow = module
        .functions()
        .find_map(|(_, function)| (function.kind() == FunctionKind::Arrow).then_some(function))
        .expect("arrow function must remain live");

    assert_eq!(
        print_function(arrow),
        "bb0:\n  %0 = load_global \"arguments\"\n  return %0"
    );
}

#[test]
fn lowers_a_block_bodied_arrow_function() {
    let module =
        lower_javascript_module("const answer = () => { return 42; console.log('unreachable'); };")
            .unwrap();

    assert_eq!(
        print_module(&module),
        concat!(
            "module {\n",
            "  binding @0 const \"answer\"\n",
            "\n",
            "  function @0 entry {\n",
            "    bb0:\n",
            "      %0 = create_function @1\n",
            "      initialize_binding @0, %0\n",
            "  }\n",
            "\n",
            "  function @1 arrow {\n",
            "    bb0:\n",
            "      %0 = constant 42\n",
            "      return %0\n",
            "  }\n",
            "}",
        )
    );
}

#[test]
fn lowers_a_function_local_const_binding() {
    let module =
        lower_javascript_module("const make = () => { const value = 42; return value; };").unwrap();

    assert_eq!(
        print_module(&module),
        concat!(
            "module {\n",
            "  binding @0 const \"make\"\n",
            "  binding @1 const \"value\"\n",
            "\n",
            "  function @0 entry {\n",
            "    bb0:\n",
            "      %0 = create_function @1\n",
            "      initialize_binding @0, %0\n",
            "  }\n",
            "\n",
            "  function @1 arrow {\n",
            "    bb0:\n",
            "      %0 = constant 42\n",
            "      initialize_binding @1, %0\n",
            "      %1 = load_binding @1\n",
            "      return %1\n",
            "  }\n",
            "}",
        )
    );
}

#[test]
fn lowers_a_capture_of_a_function_local_binding() {
    let module =
        lower_javascript_module("const outer = () => { const value = 42; return () => value; };")
            .unwrap();

    assert_eq!(
        print_module(&module),
        concat!(
            "module {\n",
            "  binding @0 const \"outer\"\n",
            "  binding @1 const \"value\"\n",
            "\n",
            "  function @0 entry {\n",
            "    bb0:\n",
            "      %0 = create_function @1\n",
            "      initialize_binding @0, %0\n",
            "  }\n",
            "\n",
            "  function @1 arrow {\n",
            "    bb0:\n",
            "      %0 = constant 42\n",
            "      initialize_binding @1, %0\n",
            "      %1 = create_function @2\n",
            "      return %1\n",
            "  }\n",
            "\n",
            "  function @2 arrow {\n",
            "    bb0:\n",
            "      %0 = load_binding @1\n",
            "      return %0\n",
            "  }\n",
            "}",
        )
    );
}

#[test]
fn instantiates_a_function_local_var_before_the_body() {
    let module =
        lower_javascript_module("const read = () => { return value; var value = 42; };").unwrap();

    assert_eq!(
        print_module(&module),
        concat!(
            "module {\n",
            "  binding @0 const \"read\"\n",
            "  binding @1 var \"value\"\n",
            "\n",
            "  function @0 entry {\n",
            "    bb0:\n",
            "      %0 = create_function @1\n",
            "      initialize_binding @0, %0\n",
            "  }\n",
            "\n",
            "  function @1 arrow {\n",
            "    bb0:\n",
            "      %0 = constant undefined\n",
            "      initialize_binding @1, %0\n",
            "      %1 = load_binding @1\n",
            "      return %1\n",
            "  }\n",
            "}",
        )
    );
}

#[test]
fn returns_undefined_from_a_fallthrough_arrow_function() {
    let module = lower_javascript_module("const noop = () => {};").unwrap();

    assert_eq!(
        print_module(&module),
        concat!(
            "module {\n",
            "  binding @0 const \"noop\"\n",
            "\n",
            "  function @0 entry {\n",
            "    bb0:\n",
            "      %0 = create_function @1\n",
            "      initialize_binding @0, %0\n",
            "  }\n",
            "\n",
            "  function @1 arrow {\n",
            "    bb0:\n",
            "      %0 = constant undefined\n",
            "      return %0\n",
            "  }\n",
            "}",
        )
    );
}

#[test]
fn lowers_an_outer_binding_reference() {
    let module = lower_javascript_module("const value = 42; const read = () => value;").unwrap();

    assert_eq!(
        print_module(&module),
        concat!(
            "module {\n",
            "  binding @0 const \"value\"\n",
            "  binding @1 const \"read\"\n",
            "\n",
            "  function @0 entry {\n",
            "    bb0:\n",
            "      %0 = constant 42\n",
            "      initialize_binding @0, %0\n",
            "      %1 = create_function @1\n",
            "      initialize_binding @1, %1\n",
            "  }\n",
            "\n",
            "  function @1 arrow {\n",
            "    bb0:\n",
            "      %0 = load_binding @0\n",
            "      return %0\n",
            "  }\n",
            "}",
        )
    );
}
#[test]
fn lowers_an_expression_bodied_arrow_function() {
    let module = lower_javascript_module("const answer = () => 42;").unwrap();

    assert_eq!(
        print_module(&module),
        concat!(
            "module {\n",
            "  binding @0 const \"answer\"\n",
            "\n",
            "  function @0 entry {\n",
            "    bb0:\n",
            "      %0 = create_function @1\n",
            "      initialize_binding @0, %0\n",
            "  }\n",
            "\n",
            "  function @1 arrow {\n",
            "    bb0:\n",
            "      %0 = constant 42\n",
            "      return %0\n",
            "  }\n",
            "}",
        )
    );
}

#[test]
fn lowers_an_ordinary_function_expression() {
    let module = lower_javascript_module("const answer = function () { return 42; };").unwrap();

    assert_eq!(
        print_module(&module),
        concat!(
            "module {\n",
            "  binding @0 const \"answer\"\n",
            "\n",
            "  function @0 entry {\n",
            "    bb0:\n",
            "      %0 = create_function @1\n",
            "      initialize_binding @0, %0\n",
            "  }\n",
            "\n",
            "  function @1 ordinary {\n",
            "    bb0:\n",
            "      %0 = constant 42\n",
            "      return %0\n",
            "  }\n",
            "}",
        )
    );
}
