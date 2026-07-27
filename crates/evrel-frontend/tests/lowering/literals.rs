//! Literal lowering.

use super::*;

#[test]
fn lowers_bigint_literals_to_canonical_decimal_values() {
    let module = lower_javascript_module("1n; 0xffn;").unwrap();

    assert_eq!(
        print_entry_function(&module),
        "bb0:\n  %0 = constant 1n\n  %1 = constant 255n"
    );
}

#[test]
fn lowers_regexp_literals_with_canonical_flags() {
    let module = lower_javascript_module("/a+/ig;").unwrap();

    assert_eq!(print_entry_function(&module), "bb0:\n  %0 = regexp /a+/gi");
}

#[test]
fn lowers_meta_properties() {
    let module =
        lower_javascript_module("import.meta; function construct() { return new.target; }")
            .unwrap();
    let output = print_module(&module);

    assert!(output.contains("= import.meta"));
    assert!(output.contains("= new.target"));
}

#[test]
fn lowers_template_substitutions_as_ordered_regions() {
    let module = lower_javascript_module("`Hello, ${name()} #${count}!`;").unwrap();
    let output = print_entry_function(&module);

    assert!(output.contains("template [\"Hello, \", region @1, \" #\", region @2, \"!\"]"));
    assert!(output.contains("region @1"));
    assert!(output.contains("region @2"));
    assert!(
        output.find("load_global \"name\"").unwrap()
            < output.find("load_global \"count\"").unwrap()
    );
}

#[test]
fn lowers_dynamic_import_source_and_options_in_order() {
    let module =
        lower_javascript_module("import(specifier(), { with: { type: \"json\" } });").unwrap();
    let function = module
        .function(module.entry_function())
        .expect("entry function must remain live");
    let import = function
        .blocks()
        .flat_map(|(_, block)| block.operations())
        .find_map(|operation| {
            let operation = function
                .operation(*operation)
                .expect("block must reference a live operation");

            matches!(operation.kind(), OperationKind::DynamicImport(_)).then_some(operation)
        })
        .expect("dynamic import operation must be emitted");
    let output = print_entry_function(&module);

    assert!(
        import
            .kind()
            .intrinsic_effects()
            .may_have_observable_effects()
    );
    assert!(!import.kind().intrinsic_effects().may_throw());
    assert_eq!(import.operands().len(), 2);
    assert!(import.operands()[0] < import.operands()[1]);
    assert!(output.contains("dynamic_import %1, options:"));
}
