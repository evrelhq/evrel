//! TypeScript erasure and rejection behavior.

use super::*;

#[test]
fn erases_typescript_type_annotations() {
    let module = lower_typescript_module("const value: number = 42; value;").unwrap();
    let output = print_entry_function(&module);

    assert!(output.contains("constant 42"));
    assert!(output.contains("initialize_binding @0"));
    assert!(output.contains("load_binding @0"));
}

#[test]
fn erases_typescript_expression_wrappers() {
    let typescript = lower_typescript_module(
        "
            const a = value as unknown;
            const b = value satisfies unknown;
            const c = <unknown>value;
            const d = value!;
            const e = identity<string>;
            ",
    )
    .unwrap();

    let javascript = lower_javascript_module(
        "
            const a = value;
            const b = value;
            const c = value;
            const d = value;
            const e = identity;
            ",
    )
    .unwrap();

    assert_eq!(
        print_entry_function(&typescript),
        print_entry_function(&javascript),
    );
}

#[test]
fn erases_type_only_declarations() {
    let typescript = lower_typescript_module(
        "
            type Identifier = string;
            interface Entity {
                id: Identifier;
            }

            export type Result = Entity;
            export interface Options {
                enabled: boolean;
            }

            const value = 42;
            ",
    )
    .unwrap();

    let javascript = lower_javascript_module("const value = 42;").unwrap();

    assert_eq!(print_module(&typescript), print_module(&javascript));
}

#[test]
fn erases_type_only_imports_and_exports() {
    let typescript = lower_typescript_module(
        r#"
            import type { Model } from "types";
            import { type Config, runtime } from "dependency";

            export type { Model };
            export { type Config, runtime };
            export type { Remote } from "remote-types";
            export { type Hidden, visible } from "remote-values";

            runtime;
            "#,
    )
    .unwrap();

    let javascript = lower_javascript_module(
        r#"
            import { runtime } from "dependency";

            export { runtime };
            export { visible } from "remote-values";

            runtime;
            "#,
    )
    .unwrap();

    assert_eq!(print_module(&typescript), print_module(&javascript));
}

#[test]
fn erases_ambient_typescript_declarations() {
    let typescript = lower_typescript_module(
        r#"
            declare const brand: unique symbol;
            declare function read(): string;
            declare class Service {}
            declare enum Mode {}

            declare module "dependency" {
                export interface Configuration {}
            }
            "#,
    )
    .unwrap();

    let javascript = lower_javascript_module("").unwrap();

    assert_eq!(print_module(&typescript), print_module(&javascript));
}

#[test]
fn erases_type_only_import_equals_declarations() {
    let typescript = lower_typescript_module(
        r#"
            import type Dependency = require("dependency");
            type Result = Dependency.Value;
            "#,
    )
    .unwrap();

    let javascript = lower_javascript_module("").unwrap();

    assert_eq!(print_module(&typescript), print_module(&javascript));
}

#[test]
fn rejects_runtime_import_equals_declarations() {
    assert!(matches!(
        lower_typescript_module(r#"import Dependency = require("dependency");"#),
        Err(super::FrontendError::UnsupportedStatement),
    ));
}
