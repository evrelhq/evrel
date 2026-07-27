use evrel_codegen_js::generate;
use evrel_frontend::lower_source_file;
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::process::Command;

pub fn generate_source(source: &str) -> String {
    let module = lower_source_file("input.js", source).expect("source must lower");
    let output = generate(&module).expect("lowered module must generate");
    assert_reparses(&output);
    output
}

pub fn assert_reparses(source: &str) {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
    assert!(
        parsed.diagnostics.is_empty(),
        "generated JavaScript did not reparse:\n{:#?}\n\n{source}",
        parsed.diagnostics,
    );
}

pub fn execute_module(source: &str) -> String {
    let output = Command::new("node")
        .args(["--input-type=module", "--eval", source])
        .output()
        .expect("Node.js 24 must be available for semantic codegen tests");
    assert!(
        output.status.success(),
        "generated module failed:\n{}\n\n{source}",
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8(output.stdout)
        .expect("test output must be UTF-8")
        .trim()
        .to_owned()
}

pub fn assert_same_result(source: &str) {
    let expected = execute_module(source);
    let generated = generate_source(source);
    let actual = execute_module(&generated);
    assert_eq!(
        actual, expected,
        "generated module changed behavior:\n{generated}"
    );
}
