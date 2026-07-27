//! Module lowering.

use super::*;

#[test]
fn records_bare_static_imports_without_executable_operations() {
    let module = lower_javascript_module(
        "import \"./setup.js\";
             import {} from \"./empty.js\";",
    )
    .unwrap();
    let sources = module
        .imports()
        .iter()
        .map(|import| import.source())
        .collect::<Vec<_>>();
    let entry = module
        .function(module.entry_function())
        .expect("entry function must remain live");

    assert_eq!(sources, ["./setup.js", "./empty.js"]);
    assert_eq!(entry.operation_count(), 0);
    assert!(print_module(&module).contains("import \"./setup.js\""));
}

#[test]
fn records_default_imports_as_live_module_bindings() {
    let module = lower_javascript_module(
        "import value from \"./dependency.js\";
             consume(value);",
    )
    .unwrap();
    let [import] = module.imports() else {
        panic!("expected one module import");
    };
    let binding = import
        .binding()
        .expect("default import must have a binding");
    let output = print_entry_function(&module);

    assert_eq!(import.source(), "./dependency.js");
    assert_eq!(module.binding(binding).unwrap().kind(), BindingKind::Import);
    assert!(output.contains("load_binding @0"));
    assert!(!output.contains("initialize_binding @0"));
    assert!(print_module(&module).contains("import default @0 from \"./dependency.js\""));
}

#[test]
fn records_namespace_imports_as_live_module_bindings() {
    let module = lower_javascript_module(
        "import * as dependency from \"./dependency.js\";
             dependency.read();",
    )
    .unwrap();
    let [import] = module.imports() else {
        panic!("expected one module import");
    };
    let binding = import
        .binding()
        .expect("namespace import must have a binding");
    let output = print_entry_function(&module);

    assert_eq!(module.binding(binding).unwrap().kind(), BindingKind::Import);
    assert!(output.contains("load_binding @0"));
    assert!(output.contains("call %0[\"read\"]"));
    assert!(!output.contains("initialize_binding @0"));
    assert!(print_module(&module).contains("import namespace @0 from \"./dependency.js\""));
}

#[test]
fn records_identifier_and_string_named_imports() {
    let module = lower_javascript_module(
        "import { read as load, \"remote-name\" as remote } from \"./dependency.js\";
             load();
             remote();",
    )
    .unwrap();
    let imported_names = module
        .imports()
        .iter()
        .map(|import| {
            import
                .imported_name()
                .expect("named import must select an export")
                .as_str()
        })
        .collect::<Vec<_>>();
    let output = print_module(&module);

    assert_eq!(imported_names, ["read", "remote-name"]);
    assert!(output.contains("import { read as @0 } from \"./dependency.js\""));
    assert!(output.contains("import { \"remote-name\" as @1 } from \"./dependency.js\""));
    assert!(!output.contains("initialize_binding @0"));
    assert!(!output.contains("initialize_binding @1"));
}

#[test]
fn records_static_import_attributes() {
    let module = lower_javascript_module(
        "import data from \"./data.json\" with {
                type: \"json\",
                \"resolution-mode\": \"strict\"
            };",
    )
    .unwrap();
    let [import] = module.imports() else {
        panic!("expected one module import");
    };
    let attributes = import.attributes();
    let output = print_module(&module);

    assert_eq!(attributes.len(), 2);
    assert_eq!(attributes[0].key().as_str(), "type");
    assert_eq!(attributes[0].value(), "json");
    assert_eq!(attributes[1].key().as_str(), "resolution-mode");
    assert_eq!(attributes[1].value(), "strict");
    assert!(output.contains("with { type: \"json\", \"resolution-mode\": \"strict\" }"));
}

#[test]
fn records_local_named_exports_without_executable_export_operations() {
    let module = lower_javascript_module(
        "const value = 42;
             export { value, value as answer };",
    )
    .unwrap();
    let exported_names = module
        .exports()
        .iter()
        .map(|export| {
            export
                .exported_name()
                .expect("local export must have an exported name")
                .as_str()
        })
        .collect::<Vec<_>>();
    let output = print_module(&module);

    assert_eq!(exported_names, ["value", "answer"]);
    assert_eq!(module.exports()[0].binding(), module.exports()[1].binding());
    assert!(output.contains("export @0 as value"));
    assert!(output.contains("export @0 as answer"));
    assert_eq!(output.matches("initialize_binding @0").count(), 1);
}

#[test]
fn records_and_lowers_exported_variable_declarations() {
    let module = lower_javascript_module(
        "export const value = 42;
             export const { answer } = source;",
    )
    .unwrap();
    let exported_names = module
        .exports()
        .iter()
        .map(|export| {
            export
                .exported_name()
                .expect("local export must have an exported name")
                .as_str()
        })
        .collect::<Vec<_>>();
    let output = print_module(&module);

    assert_eq!(exported_names, ["value", "answer"]);
    assert!(output.contains("export @0 as value"));
    assert!(output.contains("export @1 as answer"));
    assert!(output.contains("initialize_binding @0"));
    assert!(output.contains("destructure_binding.initialize"));
}

#[test]
fn records_and_instantiates_exported_function_declarations() {
    let module = lower_javascript_module(
        "answer();
             export function answer() { return 42; }",
    )
    .unwrap();
    let [export] = module.exports() else {
        panic!("expected one module export");
    };
    let output = print_module(&module);

    assert_eq!(
        export
            .exported_name()
            .expect("local export must have an exported name")
            .as_str(),
        "answer"
    );
    assert_eq!(
        module
            .binding(export.binding().expect("local export must have a binding"))
            .unwrap()
            .kind(),
        BindingKind::Function
    );
    assert_eq!(output.matches("create_function @1").count(), 1);
    assert!(output.contains("export @0 as answer"));
    assert!(output.contains("initialize_binding @0"));
}

#[test]
fn records_named_indirect_exports_without_local_bindings() {
    let module = lower_javascript_module(
        r#"export {
                read as load,
                "remote-name" as remote
            } from "./dependency.js" with { type: "json" };"#,
    )
    .unwrap();
    let [load, remote] = module.exports() else {
        panic!("expected two indirect exports");
    };
    let output = print_module(&module);

    assert_eq!(load.source(), Some("./dependency.js"));
    assert_eq!(load.imported_name().unwrap().as_str(), "read");
    assert_eq!(
        load.exported_name()
            .expect("indirect export must have an exported name")
            .as_str(),
        "load"
    );
    assert_eq!(load.binding(), None);
    assert_eq!(remote.imported_name().unwrap().as_str(), "remote-name");
    assert_eq!(
        remote
            .exported_name()
            .expect("indirect export must have an exported name")
            .as_str(),
        "remote"
    );
    assert_eq!(load.attributes()[0].value(), "json");
    assert!(
        output.contains("export { read as load } from \"./dependency.js\" with { type: \"json\" }")
    );
}

#[test]
fn records_star_exports_without_an_exported_name() {
    let module =
        lower_javascript_module(r#"export * from "./dependency.js" with { type: "json" };"#)
            .unwrap();
    let [export] = module.exports() else {
        panic!("expected one star export");
    };
    let entry = module
        .function(module.entry_function())
        .expect("entry function must remain live");
    let output = print_module(&module);

    assert_eq!(export.source(), Some("./dependency.js"));
    assert_eq!(export.exported_name(), None);
    assert_eq!(export.binding(), None);
    assert_eq!(export.attributes()[0].value(), "json");
    assert_eq!(entry.operation_count(), 0);
    assert!(output.contains("export * from \"./dependency.js\" with { type: \"json\" }"));
}

#[test]
fn records_namespace_exports_without_local_bindings() {
    let module = lower_javascript_module(
        r#"export * as dependency
               from "./dependency.js"
               with { type: "json" };"#,
    )
    .unwrap();
    let [export] = module.exports() else {
        panic!("expected one namespace export");
    };
    let entry = module
        .function(module.entry_function())
        .expect("entry function must remain live");
    let output = print_module(&module);

    assert_eq!(export.source(), Some("./dependency.js"));
    assert_eq!(
        export
            .exported_name()
            .expect("namespace export must have an exported name")
            .as_str(),
        "dependency"
    );
    assert_eq!(export.imported_name(), None);
    assert_eq!(export.binding(), None);
    assert_eq!(export.attributes()[0].value(), "json");
    assert_eq!(entry.operation_count(), 0);
    assert!(
        output.contains("export * as dependency from \"./dependency.js\" with { type: \"json\" }")
    );
}

#[test]
fn lowers_default_export_expressions_at_their_source_position() {
    let module = lower_javascript_module("before(); export default produce(); after();").unwrap();
    let [export] = module.exports() else {
        panic!("expected one default export");
    };
    let output = print_module(&module);

    assert_eq!(
        export
            .exported_name()
            .expect("default export must have an exported name")
            .as_str(),
        "default"
    );
    assert!(export.binding().is_some());
    assert!(output.contains("binding @0 const \"*default*\""));
    assert!(output.contains("export @0 as default"));

    let before = output.find("load_global \"before\"").unwrap();
    let produce = output.find("load_global \"produce\"").unwrap();
    let initialize = output.find("initialize_binding @0").unwrap();
    let after = output.find("load_global \"after\"").unwrap();

    assert!(before < produce);
    assert!(produce < initialize);
    assert!(initialize < after);
}

#[test]
fn records_and_instantiates_named_default_functions() {
    let module =
        lower_javascript_module("answer(); export default function answer() { return 42; }")
            .unwrap();
    let [export] = module.exports() else {
        panic!("expected one default export");
    };
    let output = print_module(&module);

    assert_eq!(
        export
            .exported_name()
            .expect("default export must have an exported name")
            .as_str(),
        "default"
    );
    assert!(output.contains("binding @0 function \"answer\""));
    assert!(output.contains("export @0 as default"));
    assert_eq!(output.matches("create_function @1").count(), 1);

    let initialize = output.find("initialize_binding @0").unwrap();
    let load = output.find("load_binding @0").unwrap();

    assert!(initialize < load);
    assert!(!output.contains("\"*default*\""));
}

#[test]
fn instantiates_anonymous_default_functions() {
    let module = lower_javascript_module("export default function() { return 42; }").unwrap();
    let [export] = module.exports() else {
        panic!("expected one default export");
    };
    let output = print_module(&module);

    assert_eq!(
        export
            .exported_name()
            .expect("default export must have an exported name")
            .as_str(),
        "default"
    );
    assert!(output.contains("binding @0 function \"*default*\""));
    assert!(output.contains("export @0 as default"));
    assert_eq!(output.matches("create_function @1").count(), 1);
    assert!(output.contains("initialize_binding @0"));
}
