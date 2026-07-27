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
            "bb4:\n",
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
            "  %1 = load_global \"work\" [unwind: @0]\n",
            "  %2 = call %1, args: [] [unwind: @0]\n",
            "  jump bb1 [unwind: @0]",
        )
    );
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
