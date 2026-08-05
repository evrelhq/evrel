//! Exception lowering.

use super::*;

#[test]
fn lowers_throw_as_an_abrupt_terminator() {
    let module = lower_javascript_module("throw createError(); unreachable();").unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "bb0:\n",
            "  %0 = load_global \"createError\"\n",
            "  %1 = call %0, args: []\n",
            "  throw %1",
        )
    );
}

#[test]
fn lowers_try_catch_with_an_exception_parameter() {
    let module =
        lower_javascript_module("try { work(); } catch (error) { recover(error); } after();")
            .unwrap();

    assert_eq!(
        print_entry_function(&module),
        concat!(
            "handler @0 catch entry: bb2\n",
            "\n",
            "region @1 results: 1, parent: region @0, owner: op @8\n",
            "bb6:\n",
            "  %4 = load_binding @0\n",
            "  region_yield %4\n",
            "\n",
            "bb0:\n",
            "  try body: bb3, catch: bb2, finally: none, completion: bb1\n",
            "\n",
            "bb1:\n",
            "  %6 = load_global \"after\"\n",
            "  %7 = call %6, args: []\n",
            "\n",
            "bb2(%0 [exception]):\n",
            "  initialize_binding @0, %0\n",
            "  %3 = load_global \"recover\"\n",
            "  %5 = call %3, args: [region @1]\n",
            "  jump bb1\n",
            "\n",
            "bb3:\n",
            "  invoke load_global \"work\", normal: bb4, exception: bb2\n",
            "\n",
            "bb4(%1 [produced]):\n",
            "  invoke call %1, args: [], normal: bb5, exception: bb2\n",
            "\n",
            "bb5(%2 [produced]):\n",
            "  jump bb1",
        )
    );
}

#[test]
fn makes_locally_handled_throwing_operations_explicit() {
    let module = lower_javascript_module("try { object[key]; } catch (error) {}").unwrap();
    let output = print_entry_function(&module);

    assert!(
        output.contains("invoke load_global \"object\", normal:"),
        "{output}",
    );
    assert!(
        output.contains("invoke load_global \"key\", normal:"),
        "{output}",
    );
    assert!(output.contains("invoke load_property"), "{output}");
    assert!(output.contains("exception: bb2"), "{output}");
}

#[test]
fn makes_a_locally_caught_throw_edge_explicit() {
    let module =
        lower_javascript_module("try { throw error; } catch (caught) { use(caught); }").unwrap();
    let output = print_entry_function(&module);

    assert!(output.contains("throw %1, exception: bb2"), "{output}");
    assert!(output.contains("bb2(%0 [exception]):"), "{output}");
}

#[test]
fn lifts_region_exceptions_to_the_owning_operation() {
    let module = lower_javascript_module("try { [possiblyMissing]; } catch (error) {}").unwrap();
    let output = print_entry_function(&module);

    assert!(
        output.contains("invoke array_literal [region @1], normal:"),
        "{output}",
    );
    assert!(output.contains("exception: bb2"), "{output}");

    let region = output
        .split("region @1")
        .nth(1)
        .expect("array element region must be printed")
        .split("bb0:")
        .next()
        .expect("function body must follow the inline region");

    assert!(
        region.contains("load_global \"possiblyMissing\""),
        "{output}"
    );
    assert!(!region.contains("[unwind:"), "{output}");
}

#[test]
fn lowers_a_destructured_catch_parameter() {
    let module = lower_javascript_module(
        "try { work(); } catch ({ message, ...rest }) { recover(message); }",
    )
    .unwrap();
    let output = print_entry_function(&module);

    assert!(output.contains("destructure_binding.initialize {\"message\": @0, ...@1}, %0"));
    assert!(output.contains("load_binding @0"));
}

#[test]
fn classifies_catch_parameter_bindings_separately() {
    let module = lower_javascript_module("try { work(); } catch ({ message }) {}").unwrap();

    let (_, binding) = module.bindings().next().unwrap();

    assert_eq!(binding.name(), "message");
    assert_eq!(binding.kind(), BindingKind::Catch);
}

#[test]
fn nests_try_catch_handlers() {
    let module = lower_javascript_module(
        "
            try {
                try {
                    work();
                } catch (inner) {
                    recoverInner(inner);
                }
            } catch (outer) {
                recoverOuter(outer);
            }
            ",
    )
    .unwrap();

    let function = module
        .function(module.entry_function())
        .expect("entry function must remain live");
    let handlers = function.exception_handlers().collect::<Vec<_>>();

    assert_eq!(handlers.len(), 2);

    let (outer_id, outer) = handlers[0];
    let (_, inner) = handlers[1];

    assert_eq!(outer.parent(), None);
    assert_eq!(inner.parent(), Some(outer_id));
}

#[test]
fn lowers_try_finally() {
    let module =
        lower_javascript_module("try { work(); } finally { cleanup(); } after();").unwrap();
    let output = print_entry_function(&module);
    let function = module.function(module.entry_function()).unwrap();
    let handlers = function.exception_handlers().collect::<Vec<_>>();

    assert_eq!(handlers.len(), 1);
    assert_eq!(handlers[0].1.kind(), ExceptionHandlerKind::Finally);
    assert_eq!(handlers[0].1.parent(), None);

    assert!(output.contains("try body:"));
    assert!(output.contains("catch: none"));
    assert!(output.contains("finally: bb"));
    assert!(output.contains("load_global \"cleanup\""));
    assert!(output.contains("enter_finally normal, target:"));
    assert!(output.contains("enter_finally throw %"));
    assert!(output.contains("resume_completion %"));
}

#[test]
fn models_return_and_throw_completions_through_finally() {
    let module = lower_javascript_module(
        "function returns() { try { return 1; } finally { cleanup(); } }\n\
         function throws() { try { throw error; } finally { cleanup(); } }",
    )
    .unwrap();
    let output = print_module(&module);

    assert!(output.contains("enter_finally return %"), "{output}");
    assert!(output.contains("enter_finally throw %"), "{output}");
    assert!(output.contains("resume_completion %"), "{output}");
    assert!(output.contains("return: bb"), "{output}");
    assert!(output.contains("throw: bb"), "{output}");
}

#[test]
fn models_break_and_continue_completions_through_finally() {
    let module = lower_javascript_module(
        "outer: for (;;) {\n\
           try { if (condition) break outer; continue outer; }\n\
           finally { cleanup(); }\n\
         }",
    )
    .unwrap();
    let output = print_entry_function(&module);

    assert!(output.contains("enter_finally break bb"), "{output}");
    assert!(output.contains("enter_finally continue bb"), "{output}");
    assert!(output.contains("resume_completion %"), "{output}");
}

#[test]
fn keeps_a_loop_local_break_inside_the_try() {
    let module = lower_javascript_module(
        "try { while (condition) { break; } afterLoop(); } finally { cleanup(); }",
    )
    .unwrap();
    let output = print_entry_function(&module);

    assert!(!output.contains("enter_finally break bb"), "{output}");
}

#[test]
fn nests_catch_inside_finally() {
    let module = lower_javascript_module(
        "try { work(); } catch (error) { recover(error); } finally { cleanup(); }",
    )
    .unwrap();
    let output = print_entry_function(&module);
    let function = module.function(module.entry_function()).unwrap();
    let handlers = function.exception_handlers().collect::<Vec<_>>();

    assert_eq!(handlers.len(), 2);

    let (finally_id, finally_handler) = handlers[0];
    let (_, catch_handler) = handlers[1];

    assert_eq!(finally_handler.kind(), ExceptionHandlerKind::Finally);
    assert_eq!(finally_handler.parent(), None);
    assert_eq!(catch_handler.kind(), ExceptionHandlerKind::Catch);
    assert_eq!(catch_handler.parent(), Some(finally_id));

    assert!(output.contains("try body:"));
    assert!(output.contains("catch: bb"));
    assert!(output.contains("finally: bb"));
    assert!(output.contains("load_global \"cleanup\""));
}
