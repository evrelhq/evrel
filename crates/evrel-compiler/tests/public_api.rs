use evrel_compiler::{
    CompileInput, ModuleKey, ModuleRequest, ModuleRequestKind, ProgramInput, ProgramModuleInput,
    ResolvedModuleRequest, ResolvedModuleTarget, compile, compile_program,
};
use rayon::ThreadPoolBuilder;

#[test]
fn compiles_javascript() {
    let output = compile(CompileInput::new(
        "input.js",
        "export const answer = 40 + 2;",
    ))
    .unwrap();

    assert!(!output.code().is_empty());
}

#[test]
fn erases_typescript_syntax() {
    let output = compile(CompileInput::new(
        "input.ts",
        "export const answer: number = 42;",
    ))
    .unwrap();

    assert!(!output.code().contains(": number"));
}

#[test]
fn infers_the_source_language_from_the_filename() {
    assert!(compile(CompileInput::new("component.tsx", "const view = <span />;")).is_ok());
    assert!(compile(CompileInput::new("input.txt", "const value = 42;")).is_err());
}

#[test]
fn compiles_a_host_resolved_program() {
    let dependency = ModuleKey::new("file:///dependency.js");
    let disconnected = ModuleKey::new("file:///disconnected.js");
    let entry = ModuleKey::new("file:///entry.js");
    let input = ProgramInput::new(
        [
            ProgramModuleInput::new(
                dependency.clone(),
                "dependency.js",
                "export const answer = 42;",
            ),
            ProgramModuleInput::new(
                entry.clone(),
                "entry.js",
                "import { answer } from './dependency.js'; console.log(answer);",
            )
            .with_resolved_requests([ResolvedModuleRequest::new(
                ModuleRequest::new(ModuleRequestKind::StaticImport, "./dependency.js", []),
                ResolvedModuleTarget::Internal(dependency.clone()),
            )]),
            ProgramModuleInput::new(
                disconnected.clone(),
                "disconnected.js",
                "console.log('unreachable');",
            ),
        ],
        [entry.clone()],
    );

    let output = compile_program(input).unwrap();

    assert!(output.module(&dependency).is_some());
    assert!(output.module(&entry).is_some());
    assert!(output.module(&disconnected).is_none());
}

#[test]
fn removes_unreachable_exports_but_preserves_initializer_effects() {
    let dependency = ModuleKey::new("file:///dependency.js");
    let entry = ModuleKey::new("file:///entry.js");
    let input = ProgramInput::new(
        [
            ProgramModuleInput::new(
                dependency.clone(),
                "dependency.js",
                "
                    export function used() { return 1; }
                    export function unused() { return 2; }
                    export const unusedEffect = console.log('retained-effect');
                ",
            ),
            ProgramModuleInput::new(
                entry.clone(),
                "entry.js",
                "import { used } from './dependency.js'; console.log(used());",
            )
            .with_resolved_requests([ResolvedModuleRequest::new(
                ModuleRequest::new(ModuleRequestKind::StaticImport, "./dependency.js", []),
                ResolvedModuleTarget::Internal(dependency.clone()),
            )]),
        ],
        [entry],
    );

    let output = compile_program(input).unwrap();
    let dependency = output.module(&dependency).unwrap().code();

    assert!(dependency.contains("used"));
    assert!(!dependency.contains("unused"));
    assert!(dependency.contains("retained-effect"));
}

#[test]
fn removes_an_unused_import_and_export_but_preserves_module_evaluation() {
    let dependency = ModuleKey::new("file:///dependency.js");
    let entry = ModuleKey::new("file:///entry.js");
    let input = ProgramInput::new(
        [
            ProgramModuleInput::new(
                dependency.clone(),
                "dependency.js",
                "
                    console.log('module-effect');
                    export const unused = 42;
                ",
            ),
            ProgramModuleInput::new(
                entry.clone(),
                "entry.js",
                "import { unused } from './dependency.js'; console.log('entry');",
            )
            .with_resolved_requests([ResolvedModuleRequest::new(
                ModuleRequest::new(ModuleRequestKind::StaticImport, "./dependency.js", []),
                ResolvedModuleTarget::Internal(dependency.clone()),
            )]),
        ],
        [entry.clone()],
    );

    let output = compile_program(input).unwrap();
    let dependency = output.module(&dependency).unwrap().code();
    let entry = output.module(&entry).unwrap().code();

    assert!(dependency.contains("module-effect"));
    assert!(!dependency.contains("unused"));
    assert!(entry.contains("import \"./dependency.js\""));
    assert!(!entry.contains("unused"));
}

#[test]
fn removes_an_unused_reexport_chain_but_preserves_module_evaluation() {
    let dependency = ModuleKey::new("file:///dependency.js");
    let barrel = ModuleKey::new("file:///barrel.js");
    let entry = ModuleKey::new("file:///entry.js");
    let input = ProgramInput::new(
        [
            ProgramModuleInput::new(
                dependency.clone(),
                "dependency.js",
                "
                    console.log('dependency-effect');
                    export const unused = 42;
                ",
            ),
            ProgramModuleInput::new(
                barrel.clone(),
                "barrel.js",
                "export { unused } from './dependency.js';",
            )
            .with_resolved_requests([ResolvedModuleRequest::new(
                ModuleRequest::new(ModuleRequestKind::ReExport, "./dependency.js", []),
                ResolvedModuleTarget::Internal(dependency.clone()),
            )]),
            ProgramModuleInput::new(
                entry.clone(),
                "entry.js",
                "import { unused } from './barrel.js'; console.log('entry');",
            )
            .with_resolved_requests([ResolvedModuleRequest::new(
                ModuleRequest::new(ModuleRequestKind::StaticImport, "./barrel.js", []),
                ResolvedModuleTarget::Internal(barrel.clone()),
            )]),
        ],
        [entry.clone()],
    );

    let output = compile_program(input).unwrap();
    let dependency = output.module(&dependency).unwrap().code();
    let barrel = output.module(&barrel).unwrap().code();
    let entry = output.module(&entry).unwrap().code();

    assert!(dependency.contains("dependency-effect"));
    assert!(!dependency.contains("unused"));
    assert!(barrel.contains("export {} from \"./dependency.js\""));
    assert!(!barrel.contains("unused"));
    assert!(entry.contains("import \"./barrel.js\""));
    assert!(!entry.contains("unused"));
}

#[test]
fn preserves_a_live_export_forwarded_through_an_import_binding() {
    let dependency = ModuleKey::new("file:///dependency.js");
    let barrel = ModuleKey::new("file:///barrel.js");
    let entry = ModuleKey::new("file:///entry.js");
    let input = ProgramInput::new(
        [
            ProgramModuleInput::new(
                dependency.clone(),
                "dependency.js",
                "export const value = 42;",
            ),
            ProgramModuleInput::new(
                barrel.clone(),
                "barrel.js",
                "
                    import { value } from './dependency.js';
                    export { value };
                ",
            )
            .with_resolved_requests([ResolvedModuleRequest::new(
                ModuleRequest::new(ModuleRequestKind::StaticImport, "./dependency.js", []),
                ResolvedModuleTarget::Internal(dependency.clone()),
            )]),
            ProgramModuleInput::new(
                entry.clone(),
                "entry.js",
                "import { value } from './barrel.js'; console.log(value);",
            )
            .with_resolved_requests([ResolvedModuleRequest::new(
                ModuleRequest::new(ModuleRequestKind::StaticImport, "./barrel.js", []),
                ResolvedModuleTarget::Internal(barrel.clone()),
            )]),
        ],
        [entry],
    );

    let output = compile_program(input).unwrap();

    assert!(output.module(&dependency).unwrap().code().contains("value"));
    assert!(output.module(&barrel).unwrap().code().contains("value"));
}

#[test]
fn preserves_module_bindings_visible_to_direct_eval() {
    let dependency = ModuleKey::new("file:///dependency.js");
    let entry = ModuleKey::new("file:///entry.js");
    let input = ProgramInput::new(
        [
            ProgramModuleInput::new(
                dependency.clone(),
                "dependency.js",
                "const secret = 42; console.log(eval('secret'));",
            ),
            ProgramModuleInput::new(entry.clone(), "entry.js", "import './dependency.js';")
                .with_resolved_requests([ResolvedModuleRequest::new(
                    ModuleRequest::new(ModuleRequestKind::StaticImport, "./dependency.js", []),
                    ResolvedModuleTarget::Internal(dependency.clone()),
                )]),
        ],
        [entry],
    );

    let output = compile_program(input).unwrap();

    assert!(
        output
            .module(&dependency)
            .unwrap()
            .code()
            .contains("secret")
    );
}

#[test]
fn retains_all_modules_for_an_unresolved_dynamic_import() {
    let possible_target = ModuleKey::new("file:///possible-target.js");
    let entry = ModuleKey::new("file:///entry.js");
    let input = ProgramInput::new(
        [
            ProgramModuleInput::new(
                possible_target.clone(),
                "possible-target.js",
                "globalThis.loaded = true;",
            ),
            ProgramModuleInput::new(
                entry.clone(),
                "entry.js",
                "const target = globalThis.target; void import(target);",
            ),
        ],
        [entry.clone()],
    );

    let output = compile_program(input).unwrap();

    assert!(output.module(&entry).is_some());
    assert!(output.module(&possible_target).is_some());
}

#[test]
fn retains_a_resolved_dynamic_target_and_removes_disconnected_modules() {
    let used = ModuleKey::new("file:///used.js");
    let unused = ModuleKey::new("file:///unused.js");
    let entry = ModuleKey::new("file:///entry.js");
    let input = ProgramInput::new(
        [
            ProgramModuleInput::new(used.clone(), "used.js", "globalThis.loaded = 'used';"),
            ProgramModuleInput::new(unused.clone(), "unused.js", "globalThis.loaded = 'unused';"),
            ProgramModuleInput::new(entry.clone(), "entry.js", "void import('./used.js');")
                .with_resolved_requests([ResolvedModuleRequest::new(
                    ModuleRequest::new(ModuleRequestKind::DynamicImport, "./used.js", []),
                    ResolvedModuleTarget::Internal(used.clone()),
                )]),
        ],
        [entry.clone()],
    );

    let output = compile_program(input).unwrap();

    assert!(output.module(&entry).is_some());
    assert!(output.module(&used).is_some());
    assert!(output.module(&unused).is_none());
}

#[test]
fn parallel_function_optimization_is_deterministic() {
    let mut source = String::new();

    for function in 0..24 {
        source.push_str(&format!("export function f{function}(value) {{"));

        for _ in 0..128 {
            source.push_str("value = value + 1;");
        }

        source.push_str("return value;}");
    }

    let compile_with_workers = |workers| {
        ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap()
            .install(|| {
                compile(CompileInput::new("input.js", &source))
                    .unwrap()
                    .into_code()
            })
    };

    assert_eq!(compile_with_workers(1), compile_with_workers(4));
}
