use evrel_frontend::lower_source_file;
use evrel_js_ir::{
    JsProgramIr, ModuleDependency, ModuleId, ModuleKey, ModuleRequest, ModuleRequestKind,
    ModuleTarget, OperationKind, ProgramFunctionId,
};

use super::{CallSiteKind, CallTargetCompleteness, FunctionReferenceSite, ProgramCallGraph};
use crate::js::analysis::ProgramLinkage;

#[test]
fn resolves_an_unmodified_local_function_binding() {
    let (program, module) = single_module(
        r#"
            function target() {}
            target();
        "#,
    );
    let target = created_functions(&program, module)[0];
    let graph = analyze(&program);
    let call = graph
        .callers_of(target)
        .next()
        .expect("local call must be represented");

    assert_eq!(call.kind(), CallSiteKind::Call);
    assert_eq!(call.targets().exact_function(), Some(target));
    assert!(graph.has_complete_incoming_calls(target));
    assert!(matches!(
        graph
            .references_to(target)
            .next()
            .map(|reference| reference.site()),
        Some(FunctionReferenceSite::Allocation { .. })
    ));
}

#[test]
fn merges_complete_targets_forwarded_through_control_flow() {
    let (program, module) = single_module(
        r#"
            function first() {}
            function second() {}
            const selected = condition ? first : second;
            selected();
        "#,
    );
    let targets = created_functions(&program, module);
    let graph = analyze(&program);
    let site = graph
        .sites()
        .find(|site| site.targets().functions().len() == 2)
        .expect("merged call must be represented");

    assert_eq!(
        site.targets().completeness(),
        CallTargetCompleteness::Complete
    );
    assert_eq!(site.targets().functions(), targets.as_slice());
    assert!(
        targets
            .iter()
            .all(|target| graph.has_complete_incoming_calls(*target))
    );
}

#[test]
fn retains_unknown_invocations_as_incomplete_sites() {
    let (program, module) = single_module("unknown(); object.method();");
    let graph = analyze(&program);
    let entry = ProgramFunctionId::new(
        module,
        program.module(module).unwrap().ir().entry_function(),
    );
    let sites = graph.sites_in(entry).collect::<Vec<_>>();

    assert_eq!(sites.len(), 2);
    assert!(sites.iter().all(|site| {
        site.targets().functions().is_empty()
            && site.targets().completeness() == CallTargetCompleteness::Incomplete
    }));
}

#[test]
fn records_program_entry_as_an_external_incoming_path() {
    let (program, module) = single_module("const value = 1;");
    let entry = ProgramFunctionId::new(
        module,
        program.module(module).unwrap().ir().entry_function(),
    );
    let graph = analyze(&program);

    assert!(
        graph
            .references_to(entry)
            .any(|reference| { reference.site() == FunctionReferenceSite::ProgramEntry })
    );
    assert!(!graph.has_complete_incoming_calls(entry));
}

#[test]
fn retains_known_targets_when_a_call_may_also_be_unknown() {
    let (program, module) = single_module(
        r#"
            function target() {}
            let callee = target;
            if (condition) callee = unknown;
            callee();
        "#,
    );
    let target = created_functions(&program, module)[0];
    let graph = analyze(&program);
    let site = graph
        .callers_of(target)
        .next()
        .expect("the known portion of an incomplete target set must be retained");

    assert_eq!(site.targets().functions(), [target]);
    assert_eq!(
        site.targets().completeness(),
        CallTargetCompleteness::Incomplete
    );
}

#[test]
fn resolves_constructor_invocations() {
    let (program, module) = single_module(
        r#"
            function Constructor() {}
            new Constructor();
        "#,
    );
    let target = created_functions(&program, module)[0];
    let graph = analyze(&program);
    let site = graph
        .callers_of(target)
        .next()
        .expect("constructor invocation must resolve");

    assert_eq!(site.kind(), CallSiteKind::Construct);
    assert_eq!(site.targets().exact_function(), Some(target));
}

#[test]
fn retains_super_calls_as_incomplete_invocation_sites() {
    let (program, _) = single_module(
        r#"
            class Base {}
            class Derived extends Base {
                constructor() { super(); }
            }
            new Derived();
        "#,
    );
    let graph = analyze(&program);
    let site = graph
        .sites()
        .find(|site| site.kind() == CallSiteKind::SuperCall)
        .expect("super call must remain explicit");

    assert!(site.targets().functions().is_empty());
    assert_eq!(
        site.targets().completeness(),
        CallTargetCompleteness::Incomplete
    );
}

#[test]
fn resolves_calls_through_internal_module_imports() {
    let source = lower_source_file("source.js", "export function target() {}").unwrap();
    let consumer = lower_source_file(
        "consumer.js",
        "import { target } from './source.js'; target();",
    )
    .unwrap();
    let mut program = JsProgramIr::new();
    let source_id = program.add_module(ModuleKey::new("source"), source);
    let consumer_id = program.add_module(ModuleKey::new("consumer"), consumer);
    program.add_dependency(ModuleDependency::new(
        consumer_id,
        ModuleRequest::new(ModuleRequestKind::StaticImport, "./source.js", []),
        ModuleTarget::Internal(source_id),
    ));
    let target = created_functions(&program, source_id)[0];
    let graph = analyze(&program);
    let call = graph
        .callers_of(target)
        .next()
        .expect("imported call must resolve to its source function");

    assert_eq!(call.caller().module(), consumer_id);
    assert_eq!(call.targets().exact_function(), Some(target));
    assert!(!graph.has_complete_incoming_calls(target));
    assert!(
        graph
            .references_to(target)
            .any(|reference| { matches!(reference.site(), FunctionReferenceSite::Export { .. }) })
    );
}

#[test]
fn records_jsx_component_uses_as_non_call_references() {
    let (program, module) = single_module(
        r#"
            function View() { return <main />; }
            const node = <View />;
        "#,
    );
    let target = created_functions(&program, module)[0];
    let graph = analyze(&program);
    let reference = graph
        .references_to(target)
        .find(|reference| matches!(reference.site(), FunctionReferenceSite::ValueUse { .. }))
        .expect("JSX component operand must remain inspectable");
    let FunctionReferenceSite::ValueUse { operation, .. } = reference.site() else {
        unreachable!()
    };
    let operation = program
        .module(operation.function().module())
        .unwrap()
        .ir()
        .function(operation.function().function())
        .unwrap()
        .operation(operation.operation())
        .unwrap();

    assert!(matches!(operation.kind(), OperationKind::JsxElement(_)));
    assert!(!graph.has_complete_incoming_calls(target));
}

#[test]
fn records_returned_function_values_as_incomplete_incoming_paths() {
    let (program, module) = single_module(
        r#"
            function target() {}
            function expose() { return target; }
            expose();
        "#,
    );
    let target = created_functions(&program, module)[0];
    let graph = analyze(&program);

    assert!(
        graph.references_to(target).any(|reference| {
            matches!(reference.site(), FunctionReferenceSite::ValueUse { .. })
        })
    );
    assert!(!graph.has_complete_incoming_calls(target));
}

#[test]
fn resolves_named_function_self_recursion() {
    let (program, module) = single_module(
        r#"
            const function_value = function recurse(value) {
                if (value) recurse(false);
            };
            function_value(true);
        "#,
    );
    let target = created_functions(&program, module)[0];
    let graph = analyze(&program);

    assert_eq!(graph.callers_of(target).count(), 2);
    assert!(graph.has_complete_incoming_calls(target));
}

#[test]
fn models_tagged_templates_as_invocation_sites() {
    let (program, module) = single_module(
        r#"
            function tag() {}
            tag`value`;
        "#,
    );
    let target = created_functions(&program, module)[0];
    let graph = analyze(&program);
    let site = graph
        .callers_of(target)
        .next()
        .expect("tag invocation must be represented");

    assert_eq!(site.kind(), CallSiteKind::TaggedTemplate);
    assert_eq!(site.targets().exact_function(), Some(target));
}

#[test]
fn direct_eval_exposes_visible_function_bindings() {
    let (program, module) = single_module(
        r#"
            function target() {}
            eval("target()");
        "#,
    );
    let target = created_functions(&program, module)[0];
    let graph = analyze(&program);

    assert!(
        graph.references_to(target).any(|reference| {
            matches!(reference.site(), FunctionReferenceSite::DirectEval { .. })
        })
    );
    assert!(!graph.has_complete_incoming_calls(target));
}

fn single_module(source: &str) -> (JsProgramIr, ModuleId) {
    let module_ir = lower_source_file("entry.jsx", source).unwrap();
    let mut program = JsProgramIr::new();
    let module = program.add_module(ModuleKey::new("entry"), module_ir);
    program.add_entry_module(module);

    (program, module)
}

fn analyze(program: &JsProgramIr) -> ProgramCallGraph {
    let linkage = ProgramLinkage::analyze(program);
    ProgramCallGraph::analyze(program, &linkage)
}

fn created_functions(program: &JsProgramIr, module: ModuleId) -> Vec<ProgramFunctionId> {
    let module_ir = program.module(module).unwrap().ir();
    let entry = module_ir.function(module_ir.entry_function()).unwrap();

    entry
        .operations()
        .filter_map(|(_, operation)| match operation.kind() {
            OperationKind::CreateFunction(create) => {
                Some(ProgramFunctionId::new(module, create.function()))
            }
            _ => None,
        })
        .collect()
}
