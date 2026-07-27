use evrel_compiler::{
    CompileInput, ModuleKey, ModuleRequest, ModuleRequestKind, ProgramInput, ProgramModuleInput,
    ResolvedModuleRequest, ResolvedModuleTarget, compile, compile_program,
};

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
        ],
        [entry.clone()],
    );

    let output = compile_program(input).unwrap();

    assert!(output.module(&dependency).is_some());
    assert!(output.module(&entry).is_some());
}
