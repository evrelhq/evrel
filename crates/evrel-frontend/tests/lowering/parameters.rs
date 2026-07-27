//! Parameter lowering.

use super::*;

#[test]
fn lowers_an_identifier_parameter() {
    let module = lower_javascript_module("const identity = value => value;").unwrap();

    assert_eq!(
        print_module(&module),
        concat!(
            "module {\n",
            "  binding @0 const \"identity\"\n",
            "  binding @1 parameter \"value\"\n",
            "\n",
            "  function @0 entry {\n",
            "    bb0:\n",
            "      %0 = create_function @1\n",
            "      initialize_binding @0, %0\n",
            "  }\n",
            "\n",
            "  function @1 arrow {\n",
            "    param %0 argument @1\n",
            "    \n",
            "    bb0:\n",
            "      %1 = load_binding @1\n",
            "      return %1\n",
            "  }\n",
            "}",
        )
    );
}

#[test]
fn resolves_a_parameter_that_shadows_an_outer_binding() {
    let module = lower_javascript_module("const value = 1; const read = value => value;").unwrap();

    assert_eq!(
        print_module(&module),
        concat!(
            "module {\n",
            "  binding @0 const \"value\"\n",
            "  binding @1 const \"read\"\n",
            "  binding @2 parameter \"value\"\n",
            "\n",
            "  function @0 entry {\n",
            "    bb0:\n",
            "      %0 = constant 1\n",
            "      initialize_binding @0, %0\n",
            "      %1 = create_function @1\n",
            "      initialize_binding @1, %1\n",
            "  }\n",
            "\n",
            "  function @1 arrow {\n",
            "    param %0 argument @2\n",
            "    \n",
            "    bb0:\n",
            "      %1 = load_binding @2\n",
            "      return %1\n",
            "  }\n",
            "}",
        )
    );
}

#[test]
fn lowers_an_identifier_rest_parameter() {
    let module = lower_javascript_module("const collect = (...values) => values;").unwrap();

    assert_eq!(
        print_module(&module),
        concat!(
            "module {\n",
            "  binding @0 const \"collect\"\n",
            "  binding @1 parameter \"values\"\n",
            "\n",
            "  function @0 entry {\n",
            "    bb0:\n",
            "      %0 = create_function @1\n",
            "      initialize_binding @0, %0\n",
            "  }\n",
            "\n",
            "  function @1 arrow {\n",
            "    param %0 rest @1\n",
            "    \n",
            "    bb0:\n",
            "      %1 = load_binding @1\n",
            "      return %1\n",
            "  }\n",
            "}",
        )
    );
}

#[test]
fn lowers_destructured_function_parameters() {
    let module = lower_javascript_module("const read = ({ value }, [fallback]) => value;").unwrap();

    let arrow = module
        .functions()
        .find_map(|(_, function)| (function.kind() == FunctionKind::Arrow).then_some(function))
        .unwrap();

    let names = arrow
        .parameters()
        .iter()
        .flat_map(|parameter| parameter.target().binding_ids())
        .map(|binding| module.binding(binding).unwrap().name())
        .collect::<Vec<_>>();

    assert_eq!(names, ["value", "fallback"]);
}

#[test]
fn preserves_a_function_parameter_default_as_a_region() {
    let module = lower_javascript_module("const read = (value = fallback()) => value;").unwrap();
    let arrow = module
        .functions()
        .find_map(|(_, function)| (function.kind() == FunctionKind::Arrow).then_some(function))
        .unwrap();
    let output = print_function(arrow);

    assert!(output.contains("argument @1 = region @1"));
    assert!(output.contains("region @1 results: 1, parent: region @0, owner: param 0"));
    assert!(output.contains("load_global \"fallback\""));
    assert!(output.contains("region_yield"));
}
