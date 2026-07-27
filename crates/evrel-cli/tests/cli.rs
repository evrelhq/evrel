use std::{path::PathBuf, process::Command};

use evrel_compiler::{CompileInput, compile};

#[test]
fn compiles_a_javascript_file_to_stdout() {
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/add.js");

    let output = Command::new(env!("CARGO_BIN_EXE_evrel"))
        .arg(input)
        .output()
        .expect("evrel CLI must run");

    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr),
    );

    let runtime = Command::new("node")
        .arg("-e")
        .arg(String::from_utf8(output.stdout).unwrap())
        .output()
        .expect("Node.js must run generated JavaScript");

    assert!(runtime.status.success());
    assert_eq!(runtime.stdout, b"42\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn compiles_a_tsx_file_to_stdout() {
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/component.tsx");

    let output = Command::new(env!("CARGO_BIN_EXE_evrel"))
        .arg(input)
        .output()
        .expect("evrel CLI must run");

    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(compile(CompileInput::new("output.jsx", &stdout)).is_ok());
    assert!(stdout.contains("Button"));
    assert!(stdout.contains("enabled"));
    assert!(stdout.contains("20 + 22"));
    assert!(!stdout.contains(": object"));
    assert!(output.stderr.is_empty());
}
