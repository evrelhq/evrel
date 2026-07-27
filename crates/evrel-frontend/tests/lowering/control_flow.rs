//! Control-flow lowering.

use super::*;

#[test]
fn lowers_an_if_statement() {
    let module = lower_javascript_module("if (condition) { yes(); }").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %0 = load_global \"condition\"\n",
            "  if %0, then: bb1, else: bb2, completion: bb2\n",
            "\n",
            "bb1:\n",
            "  %1 = load_global \"yes\"\n",
            "  %2 = call %1, args: []\n",
            "  jump bb2\n",
            "\n",
            "bb2:",
        )
    );
}

#[test]
fn lowers_an_if_else_statement() {
    let module = lower_javascript_module("if (condition) { yes(); } else { no(); }").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %0 = load_global \"condition\"\n",
            "  if %0, then: bb1, else: bb2, completion: bb3\n",
            "\n",
            "bb1:\n",
            "  %1 = load_global \"yes\"\n",
            "  %2 = call %1, args: []\n",
            "  jump bb3\n",
            "\n",
            "bb2:\n",
            "  %3 = load_global \"no\"\n",
            "  %4 = call %3, args: []\n",
            "  jump bb3\n",
            "\n",
            "bb3:",
        )
    );
}

#[test]
fn lowers_a_while_loop_with_break() {
    let module = lower_javascript_module("while (test) { break; } after();").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  jump bb1\n",
            "\n",
            "bb1:\n",
            "  %0 = load_global \"test\"\n",
            "  while %0, body: bb2, exit: bb3\n",
            "\n",
            "bb2:\n",
            "  jump bb3\n",
            "\n",
            "bb3:\n",
            "  %1 = load_global \"after\"\n",
            "  %2 = call %1, args: []",
        )
    );
}

#[test]
fn lowers_a_classical_for_loop_with_continue() {
    let module = lower_javascript_module(
        "for (var index = 0; index < 3; index++) { body(index); continue; } after();",
    )
    .unwrap();
    let output = print_entry_function(&module);

    assert!(
        output
            .contains("bb2:\n  for initializer: bb1, test: bb3, body: bb4, update: bb5, exit: bb6")
    );
    assert!(output.contains("bb4:\n  %5 = load_global \"body\""));
    assert!(output.contains("  jump bb5\n\nbb5:"));
    assert!(output.contains("update.increment"));
    assert!(output.contains("bb5:") && output.contains("  jump bb2"));
}

#[test]
fn lowers_an_empty_for_loop_body() {
    let module = lower_javascript_module("for (; test; ); after();").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  jump bb1\n",
            "\n",
            "bb1:\n",
            "  jump bb2\n",
            "\n",
            "bb2:\n",
            "  for initializer: bb1, test: bb3, body: bb4, update: bb5, exit: bb6\n",
            "\n",
            "bb3:\n",
            "  %0 = load_global \"test\"\n",
            "  if %0, then: bb4, else: bb6, completion: bb6\n",
            "\n",
            "bb4:\n",
            "  jump bb5\n",
            "\n",
            "bb5:\n",
            "  jump bb2\n",
            "\n",
            "bb6:\n",
            "  %1 = load_global \"after\"\n",
            "  %2 = call %1, args: []",
        )
    );
}

#[test]
fn records_per_iteration_bindings_for_a_lexical_for_header() {
    let module = lower_javascript_module("for (let index = 0; index < 3; index++) {}").unwrap();
    let function = module.function(module.entry_function()).unwrap();
    let (_, loop_operation) = function.loop_operations().next().unwrap();
    let [binding] = loop_operation.per_iteration_bindings() else {
        panic!("a for-header let binding must be per-iteration");
    };

    assert_eq!(module.binding(*binding).unwrap().name(), "index");
    assert!(print_entry_function(&module).contains(
        "for initializer: bb1, test: bb3, body: bb4, update: bb5, exit: bb6, per_iteration: [@0]"
    ));
}

#[test]
fn associates_a_captured_for_let_binding_with_per_iteration_semantics() {
    let module = lower_javascript_module(
        "for (let index = 0; index < 3; index++) { consume(() => index); }",
    )
    .unwrap();

    let entry = module.function(module.entry_function()).unwrap();
    let (_, loop_operation) = entry.loop_operations().next().unwrap();
    let [binding] = loop_operation.per_iteration_bindings() else {
        panic!("the for-header binding must be per-iteration");
    };

    let arrow = module
        .functions()
        .find_map(|(_, function)| (function.kind() == FunctionKind::Arrow).then_some(function))
        .expect("the loop body must contain an arrow function");

    let reads_binding = arrow.blocks().any(|(_, block)| {
        block.operations().iter().any(|operation| {
            matches!(
                arrow.operation(*operation).unwrap().kind(),
                OperationKind::LoadBinding(load) if load.binding() == *binding
            )
        })
    });

    assert!(reads_binding);
}

#[test]
fn excludes_var_and_const_from_per_iteration_bindings() {
    for source in [
        "for (var value = 0; test; ) {}",
        "for (const value = 0; test; ) {}",
    ] {
        let module = lower_javascript_module(source).unwrap();
        let function = module.function(module.entry_function()).unwrap();
        let (_, loop_operation) = function.loop_operations().next().unwrap();

        assert!(loop_operation.per_iteration_bindings().is_empty());
    }
}

#[test]
fn records_every_lexical_for_header_binding_in_source_order() {
    let module =
        lower_javascript_module("for (let left = 0, right = 1; left < right; left++) {}").unwrap();

    let function = module.function(module.entry_function()).unwrap();
    let (_, loop_operation) = function.loop_operations().next().unwrap();

    let names = loop_operation
        .per_iteration_bindings()
        .iter()
        .map(|binding| module.binding(*binding).unwrap().name())
        .collect::<Vec<_>>();

    assert_eq!(names, ["left", "right"]);
}

#[test]
fn records_destructured_for_let_bindings_as_per_iteration() {
    let module = lower_javascript_module("for (let [first, ...rest] = source; test; ) {}").unwrap();

    let function = module.function(module.entry_function()).unwrap();
    let (_, loop_operation) = function.loop_operations().next().unwrap();

    let names = loop_operation
        .per_iteration_bindings()
        .iter()
        .map(|binding| module.binding(*binding).unwrap().name())
        .collect::<Vec<_>>();

    assert_eq!(names, ["first", "rest"]);
}

#[test]
fn lowers_for_in_with_a_produced_property_key() {
    let module =
        lower_javascript_module("for (const key in source()) { consume(key); } after();").unwrap();
    let function = module.function(module.entry_function()).unwrap();
    let (_, loop_operation) = function.loop_operations().next().unwrap();
    let body = function.block(loop_operation.body_block()).unwrap();
    let [property_key] = body.parameters() else {
        panic!("for-in body must receive one property key");
    };

    assert_eq!(loop_operation.kind(), LoopKind::ForIn);
    assert_eq!(
        loop_operation.continue_block(),
        loop_operation.operation_block()
    );
    assert_eq!(property_key.source(), BlockParameterSource::Produced);
    assert_eq!(loop_operation.per_iteration_bindings().len(), 1);

    let output = print_entry_function(&module);
    assert_eq!(output.matches("load_global \"source\"").count(), 1);
    assert!(output.contains("for_in %1, body: bb2 [produces: 1], exit: bb3"));
    assert!(output.contains("bb2(%2 [produced]):\n  initialize_binding @0, %2"));
}

#[test]
fn evaluates_a_for_in_assignment_reference_inside_each_iteration() {
    let module = lower_javascript_module("for (target[index()] in source) { consume(); }").unwrap();
    let output = print_entry_function(&module);
    let body = output.find("[produced]):").unwrap();

    assert!(output[body..].contains("load_global \"target\""));
    assert!(output[body..].contains("load_global \"index\""));
    assert!(output[body..].contains("store_property"));
}

#[test]
fn lowers_a_for_in_destructuring_assignment() {
    let module = lower_javascript_module("let target; for ({ key: target } in source) {}").unwrap();
    let output = print_entry_function(&module);
    let body = output.find("[produced]):").unwrap();

    assert!(output[body..].contains("destructure_assignment {\"key\": @0}"));
}

#[test]
fn routes_labeled_for_in_continue_to_the_enumeration_header() {
    let module =
        lower_javascript_module("outer: for (let key in source) { continue outer; }").unwrap();
    let function = module.function(module.entry_function()).unwrap();
    let (_, loop_operation) = function.loop_operations().next().unwrap();
    let output = print_entry_function(&module);

    assert_eq!(loop_operation.label(), Some("outer"));
    assert_eq!(
        loop_operation.continue_block(),
        loop_operation.operation_block()
    );
    assert!(output.contains(
        "for_in %0, body: bb2 [produces: 1], exit: bb3, per_iteration: [@0], labels: [\"outer\"]"
    ));
}

#[test]
fn lowers_for_of_with_a_produced_iteration_value() {
    let module =
        lower_javascript_module("for (const value of source()) { consume(value); } after();")
            .unwrap();
    let function = module.function(module.entry_function()).unwrap();
    let (_, loop_operation) = function.loop_operations().next().unwrap();
    let body = function.block(loop_operation.body_block()).unwrap();
    let [iteration_value] = body.parameters() else {
        panic!("for-of body must receive one iteration value");
    };

    assert_eq!(loop_operation.kind(), LoopKind::ForOf);
    assert_eq!(loop_operation.for_of_kind(), Some(ForOfKind::Synchronous));
    assert_eq!(
        loop_operation.continue_block(),
        loop_operation.operation_block()
    );
    assert_eq!(iteration_value.source(), BlockParameterSource::Produced);
    assert_eq!(loop_operation.per_iteration_bindings().len(), 1);

    let output = print_entry_function(&module);
    assert_eq!(output.matches("load_global \"source\"").count(), 1);
    assert!(output.contains("for_of %1, body: bb2 [produces: 1], exit: bb3"));
    assert!(output.contains("bb2(%2 [produced]):\n  initialize_binding @0, %2"));
}

#[test]
fn lowers_for_await_of_with_asynchronous_iteration() {
    let module =
        lower_javascript_module("for await (const value of source) { consume(value); }").unwrap();
    let function = module.function(module.entry_function()).unwrap();
    let (_, loop_operation) = function.loop_operations().next().unwrap();
    let output = print_entry_function(&module);

    assert_eq!(loop_operation.kind(), LoopKind::ForOf);
    assert_eq!(loop_operation.for_of_kind(), Some(ForOfKind::Asynchronous));
    assert!(output.contains("for_await_of %0"));
}

#[test]
fn lowers_a_destructured_for_of_declaration() {
    let module = lower_javascript_module("for (const [first, ...rest] of source) {}").unwrap();
    let function = module.function(module.entry_function()).unwrap();
    let (_, loop_operation) = function.loop_operations().next().unwrap();
    let names = loop_operation
        .per_iteration_bindings()
        .iter()
        .map(|binding| module.binding(*binding).unwrap().name())
        .collect::<Vec<_>>();
    let output = print_entry_function(&module);

    assert_eq!(names, ["first", "rest"]);
    assert!(output.contains("destructure_binding.initialize"));
}

#[test]
fn lowers_a_for_of_destructuring_assignment() {
    let module = lower_javascript_module("let first; for ([first] of source) {}").unwrap();
    let output = print_entry_function(&module);
    let body = output.find("[produced]):").unwrap();

    assert!(output[body..].contains("destructure_assignment [@0]"));
}

#[test]
fn routes_labeled_for_of_continue_to_the_iteration_header() {
    let module =
        lower_javascript_module("outer: for (let value of source) { continue outer; }").unwrap();
    let function = module.function(module.entry_function()).unwrap();
    let (_, loop_operation) = function.loop_operations().next().unwrap();
    let output = print_entry_function(&module);

    assert_eq!(loop_operation.label(), Some("outer"));
    assert_eq!(
        loop_operation.continue_block(),
        loop_operation.operation_block()
    );
    assert!(output.contains(
        "for_of %0, body: bb2 [produces: 1], exit: bb3, per_iteration: [@0], labels: [\"outer\"]"
    ));
}

#[test]
fn lowers_continue_to_the_while_test() {
    let module = lower_javascript_module("while (test) { continue; }").unwrap();
    let output = print_entry_function(&module);

    assert_eq!(output.matches("jump bb1").count(), 2);
    assert!(output.contains("while %0, body: bb2, exit: bb3"));
}

#[test]
fn records_the_entry_of_a_multi_block_while_test() {
    let module = lower_javascript_module("while (left && right) { break; } after();").unwrap();
    let function = module.function(module.entry_function()).unwrap();
    let (_, loop_operation) = function.loop_operations().next().unwrap();

    assert_eq!(loop_operation.kind(), LoopKind::While);
    assert_ne!(
        loop_operation.test_block().unwrap(),
        loop_operation.operation_block()
    );
    assert_eq!(
        loop_operation.continue_block(),
        loop_operation.test_block().unwrap()
    );
    assert!(matches!(
        function
            .operation(
                function
                    .block(loop_operation.test_block().unwrap())
                    .unwrap()
                    .terminator()
                    .unwrap()
            )
            .unwrap()
            .kind(),
        OperationKind::If(_)
    ));
}

#[test]
fn derives_nested_loops_from_their_operations() {
    let module =
        lower_javascript_module("while (outer) { while (inner) { break; } continue; }").unwrap();
    let function = module.function(module.entry_function()).unwrap();
    let loops = function.loop_operations().collect::<Vec<_>>();

    assert_eq!(loops.len(), 2);
    assert_eq!(loops[0].1.kind(), LoopKind::While);
    assert_eq!(loops[1].1.kind(), LoopKind::While);
    assert_ne!(loops[0].1.test_block(), loops[1].1.test_block());
}

#[test]
fn lowers_a_labeled_continue() {
    let module = lower_javascript_module("outer: while (test) { continue outer; }").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  jump bb1\n",
            "\n",
            "bb1:\n",
            "  %0 = load_global \"test\"\n",
            "  while %0, body: bb2, exit: bb3, labels: [\"outer\"]\n",
            "\n",
            "bb2:\n",
            "  jump bb1\n",
            "\n",
            "bb3:",
        )
    );
}

#[test]
fn lowers_a_break_from_a_labeled_block() {
    let module =
        lower_javascript_module("section: { break section; unreachable(); } after();").unwrap();
    let function = module.function(module.entry_function()).unwrap();
    let (_, labeled_statement) = function.labeled_statements().next().unwrap();

    assert_eq!(
        labeled_statement
            .labels()
            .iter()
            .map(Box::as_ref)
            .collect::<Vec<_>>(),
        ["section"]
    );

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "labeled @0 labels: [\"section\"], body: bb1, completion: bb2\n",
            "\n",
            "bb0:\n",
            "  jump bb1\n",
            "\n",
            "bb1:\n",
            "  jump bb2\n",
            "\n",
            "bb2:\n",
            "  %0 = load_global \"after\"\n",
            "  %1 = call %0, args: []",
        )
    );
}

#[test]
fn assigns_consecutive_labels_to_the_same_loop() {
    let module =
        lower_javascript_module("outer: inner: while (test) { continue outer; continue inner; }")
            .unwrap();
    let function = module.function(module.entry_function()).unwrap();
    let (_, loop_operation) = function.loop_operations().next().unwrap();

    assert_eq!(
        loop_operation
            .labels()
            .iter()
            .map(Box::as_ref)
            .collect::<Vec<_>>(),
        ["outer", "inner"]
    );
    assert_eq!(
        loop_operation.continue_block(),
        loop_operation.test_block().unwrap()
    );
    assert_eq!(function.labeled_statements().count(), 0);
    assert!(
        print_entry_function(&module)
            .contains("while %0, body: bb2, exit: bb3, labels: [\"outer\", \"inner\"]")
    );
}

#[test]
fn unlabeled_break_skips_a_generic_labeled_statement() {
    let module =
        lower_javascript_module("while (test) { section: { break; } unreachable(); } after();")
            .unwrap();
    let function = module.function(module.entry_function()).unwrap();
    let (_, loop_operation) = function.loop_operations().next().unwrap();
    let (_, labeled_statement) = function.labeled_statements().next().unwrap();
    let body = function.block(labeled_statement.body_block()).unwrap();
    let terminator = function.operation(body.terminator().unwrap()).unwrap();
    let successors = terminator.successors();
    let [successor] = successors.as_slice() else {
        panic!("labeled body break must have one successor");
    };

    assert_eq!(successor.target().block(), loop_operation.exit_block());
}

#[test]
fn lowers_a_do_while_loop_with_continue() {
    let module =
        lower_javascript_module("do { body(); continue; } while (test); after();").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  jump bb1\n",
            "\n",
            "bb1:\n",
            "  %0 = load_global \"body\"\n",
            "  %1 = call %0, args: []\n",
            "  jump bb2\n",
            "\n",
            "bb2:\n",
            "  %2 = load_global \"test\"\n",
            "  do_while %2, body: bb1, exit: bb3\n",
            "\n",
            "bb3:\n",
            "  %3 = load_global \"after\"\n",
            "  %4 = call %3, args: []",
        )
    );
}

#[test]
fn records_a_labeled_do_while_loop() {
    let module = lower_javascript_module("outer: do { break outer; } while (test);").unwrap();
    let output = print_entry_function(&module);

    assert!(output.contains("do_while %0, body: bb1, exit: bb3, labels: [\"outer\"]"));
    assert!(output.contains("bb1:\n  jump bb3"));
}
