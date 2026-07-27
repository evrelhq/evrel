//! Suspension lowering.

use super::*;

#[test]
fn records_async_and_generator_function_modes() {
    let module = lower_javascript_module(
        "async function async_function() {}
             function* generator_function() {}
             async function* async_generator_function() {}
             const arrow = async () => 42;",
    )
    .unwrap();
    let modes = module
        .functions()
        .map(|(_, function)| function.mode())
        .collect::<Vec<_>>();

    assert_eq!(
        modes,
        [
            FunctionMode::Normal,
            FunctionMode::Async,
            FunctionMode::Generator,
            FunctionMode::AsyncGenerator,
            FunctionMode::Async,
        ]
    );
}

#[test]
fn lowers_await_as_a_suspending_value_operation() {
    let module = lower_javascript_module("const run = async () => await work();").unwrap();
    let function = module
        .functions()
        .find_map(|(_, function)| (function.mode() == FunctionMode::Async).then_some(function))
        .expect("async arrow function must remain live");
    let await_operation = function
        .blocks()
        .flat_map(|(_, block)| block.operations())
        .find_map(|operation| {
            let operation = function
                .operation(*operation)
                .expect("block must reference a live operation");

            matches!(operation.kind(), OperationKind::Await(_)).then_some(operation)
        })
        .expect("await operation must be emitted");

    assert!(await_operation.kind().intrinsic_effects().may_throw());
    assert!(await_operation.kind().intrinsic_effects().may_suspend());
    assert_eq!(
        print_function(function),
        concat!(
            "bb0:\n",
            "  %0 = load_global \"work\"\n",
            "  %1 = call %0, args: []\n",
            "  %2 = await %1\n",
            "  return %2",
        )
    );
}

#[test]
fn lowers_yield_and_delegated_yield_as_suspending_value_operations() {
    let module = lower_javascript_module(
        "function* values() {
                const resumed = yield 42;
                yield* source;
                return resumed;
            }",
    )
    .unwrap();
    let function = module
        .functions()
        .find_map(|(_, function)| (function.mode() == FunctionMode::Generator).then_some(function))
        .expect("generator function must remain live");
    let yields = function
        .blocks()
        .flat_map(|(_, block)| block.operations())
        .filter_map(|operation| {
            let operation = function
                .operation(*operation)
                .expect("block must reference a live operation");

            matches!(operation.kind(), OperationKind::Yield(_)).then_some(operation)
        })
        .collect::<Vec<_>>();

    assert_eq!(yields.len(), 2);
    assert!(
        yields
            .iter()
            .all(|operation| operation.kind().intrinsic_effects().may_throw())
    );
    assert!(
        yields
            .iter()
            .all(|operation| operation.kind().intrinsic_effects().may_suspend())
    );

    let output = print_function(function);

    assert!(output.contains("yield %0"));
    assert!(output.contains("yield* %2"));
}
