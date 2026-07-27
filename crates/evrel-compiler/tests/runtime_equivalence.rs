mod support;

use support::{
    run_runtime_fixtures, run_runtime_known_failures, run_runtime_program_fixtures,
    run_runtime_program_known_failures,
};

macro_rules! runtime_category {
    ($name:ident, $directory:literal) => {
        #[test]
        fn $name() {
            run_runtime_fixtures(concat!("tests/fixtures/runtime/", $directory));
        }
    };
}

runtime_category!(async_semantics, "async");
runtime_category!(bindings_and_scopes, "bindings");
runtime_category!(builtin_boundaries, "builtins");
runtime_category!(classes, "classes");
runtime_category!(backend_regressions, "codegen");
runtime_category!(control_flow, "control-flow");
runtime_category!(evaluation_order, "evaluation-order");
runtime_category!(exceptions, "exceptions");
runtime_category!(functions, "functions");
runtime_category!(generators, "generators");
runtime_category!(iterators, "iterators");
runtime_category!(literals, "literals");
runtime_category!(modules, "modules");
runtime_category!(objects, "objects");
runtime_category!(operators, "operators");
runtime_category!(scripts, "scripts");
runtime_category!(values_and_coercion, "values");

#[test]
fn runtime_equivalence_known_failures() {
    run_runtime_known_failures("tests/fixtures/runtime/known-failures");
}

#[test]
fn runtime_equivalence_module_graphs() {
    run_runtime_program_fixtures("tests/fixtures/runtime-programs");
}

#[test]
fn runtime_equivalence_module_graph_known_failures() {
    run_runtime_program_known_failures("tests/fixtures/runtime-programs-known-failures");
}
