use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use evrel_compiler::{CompileInput, compile};
use serde_json::{Value, json};
use tempfile::tempdir;
use wait_timeout::ChildExt;

const EXECUTION_TIMEOUT: Duration = Duration::from_secs(10);

pub fn run_runtime_fixtures(relative_directory: impl AsRef<Path>) {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_directory);
    let include_known_failures = directory
        .file_name()
        .is_some_and(|name| name == "known-failures");
    let fixtures = discover_fixtures(&directory, include_known_failures)
        .unwrap_or_else(|error| panic!("failed to discover runtime fixtures: {error}"));

    assert!(
        !fixtures.is_empty(),
        "no runtime fixtures found in {}",
        directory.display(),
    );

    let failures = fixtures
        .iter()
        .filter_map(|fixture| check_runtime_equivalence(fixture).err())
        .collect::<Vec<_>>();

    assert!(
        failures.is_empty(),
        "{} of {} runtime fixture(s) failed:\n\n{}",
        failures.len(),
        fixtures.len(),
        failures.join("\n\n"),
    );
}

/// Confirms that every fixture in a known-failure directory still exposes a
/// compiler bug. If a fixture starts passing, this test fails so the fixture is
/// promoted to the normal conformance corpus instead of silently remaining an
/// expected failure forever.
pub fn run_runtime_known_failures(relative_directory: impl AsRef<Path>) {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_directory);
    let fixtures = discover_fixtures(&directory, true)
        .unwrap_or_else(|error| panic!("failed to discover known runtime failures: {error}"));

    assert!(
        !fixtures.is_empty(),
        "no known runtime failures found in {}",
        directory.display(),
    );

    let unexpectedly_passing = fixtures
        .iter()
        .filter(|fixture| check_runtime_equivalence(fixture).is_ok())
        .map(|fixture| fixture.display().to_string())
        .collect::<Vec<_>>();

    assert!(
        unexpectedly_passing.is_empty(),
        "{} known runtime failure(s) now pass and must move into the normal fixture corpus:\n{}",
        unexpectedly_passing.len(),
        unexpectedly_passing.join("\n"),
    );
}

/// Runs directory fixtures containing an `entry.mjs` and any number of
/// relative module dependencies. Each module is compiled independently because
/// the public compiler API does not yet expose a source-to-resolved-ProgramIr
/// adapter; execution still uses the real module graph and loader semantics.
pub fn run_runtime_program_fixtures(relative_directory: impl AsRef<Path>) {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_directory);
    let fixtures = discover_program_fixtures(&directory);

    let failures = fixtures
        .iter()
        .filter_map(|fixture| check_program_runtime_equivalence(fixture).err())
        .collect::<Vec<_>>();

    assert!(
        failures.is_empty(),
        "{} of {} runtime program fixture(s) failed:\n\n{}",
        failures.len(),
        fixtures.len(),
        failures.join("\n\n"),
    );
}

/// Confirms that each known-failing module graph still exposes a compiler bug.
pub fn run_runtime_program_known_failures(relative_directory: impl AsRef<Path>) {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_directory);
    let fixtures = discover_program_fixtures(&directory);
    let unexpectedly_passing = fixtures
        .iter()
        .filter(|fixture| check_program_runtime_equivalence(fixture).is_ok())
        .map(|fixture| fixture.display().to_string())
        .collect::<Vec<_>>();

    assert!(
        unexpectedly_passing.is_empty(),
        "{} known runtime program failure(s) now pass and must move into the normal corpus:\n{}",
        unexpectedly_passing.len(),
        unexpectedly_passing.join("\n"),
    );
}

fn discover_program_fixtures(directory: &Path) -> Vec<PathBuf> {
    let mut fixtures = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()))
        .map(|entry| {
            entry
                .expect("program fixture entry must be readable")
                .path()
        })
        .filter(|path| path.is_dir() && path.join("entry.mjs").is_file())
        .collect::<Vec<_>>();
    fixtures.sort();
    assert!(
        !fixtures.is_empty(),
        "no runtime program fixtures found in {}",
        directory.display(),
    );
    fixtures
}

fn discover_fixtures(
    directory: &Path,
    include_known_failures: bool,
) -> Result<Vec<PathBuf>, String> {
    let mut pending_directories = vec![directory.to_owned()];
    let mut fixtures = Vec::new();

    while let Some(directory) = pending_directories.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|error| format!("cannot read {}: {error}", directory.display()))?;

        for entry in entries {
            let entry = entry.map_err(|error| {
                format!("cannot read an entry in {}: {error}", directory.display())
            })?;
            let path = entry.path();

            if path.is_dir()
                && (path.file_name().is_none_or(|name| name != "known-failures")
                    || include_known_failures)
            {
                pending_directories.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "mjs" || extension == "js")
            {
                fixtures.push(path);
            }
        }
    }

    fixtures.sort();

    Ok(fixtures)
}

fn check_runtime_equivalence(fixture: &Path) -> Result<(), String> {
    let source = fs::read_to_string(fixture)
        .map_err(|error| format!("{}: cannot read fixture: {error}", fixture.display()))?;
    let source_name = fixture.to_string_lossy();
    let compiled = compile(CompileInput::new(&source_name, &source))
        .map_err(|error| format!("{}: compilation failed: {error}", fixture.display()))?;
    let mode = if fixture
        .extension()
        .is_some_and(|extension| extension == "mjs")
    {
        "module"
    } else {
        "script"
    };

    let expected = execute_javascript(&source, mode)
        .map_err(|error| format!("{}: original execution failed: {error}", fixture.display()))?;
    let actual = execute_javascript(compiled.code(), mode)
        .map_err(|error| format!("{}: compiled execution failed: {error}", fixture.display()))?;

    if actual == expected {
        return Ok(());
    }

    Err(format!(
        "{}\nexpected:\n{}\nactual:\n{}\n\ncompiled output:\n{}",
        fixture.display(),
        format_execution(&expected),
        format_execution(&actual),
        compiled.code(),
    ))
}

fn check_program_runtime_equivalence(fixture: &Path) -> Result<(), String> {
    let original = read_program_sources(fixture)?;
    let compiled = original
        .iter()
        .map(|(relative_path, source)| {
            let source_name = relative_path.to_string_lossy();
            let output = compile(CompileInput::new(&source_name, source)).map_err(|error| {
                format!(
                    "{}: compilation of {} failed: {error}",
                    fixture.display(),
                    relative_path.display(),
                )
            })?;
            Ok((relative_path.clone(), output.into_code()))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let expected = execute_module_program(&original)
        .map_err(|error| format!("{}: original execution failed: {error}", fixture.display()))?;
    let actual = execute_module_program(&compiled)
        .map_err(|error| format!("{}: compiled execution failed: {error}", fixture.display()))?;

    if actual == expected {
        return Ok(());
    }

    let compiled_sources = compiled
        .iter()
        .map(|(path, source)| format!("// {}\n{source}", path.display()))
        .collect::<Vec<_>>()
        .join("\n");
    Err(format!(
        "{}\nexpected:\n{}\nactual:\n{}\n\ncompiled output:\n{}",
        fixture.display(),
        format_execution(&expected),
        format_execution(&actual),
        compiled_sources,
    ))
}

fn read_program_sources(directory: &Path) -> Result<Vec<(PathBuf, String)>, String> {
    let mut pending = vec![directory.to_owned()];
    let mut sources = Vec::new();

    while let Some(current) = pending.pop() {
        for entry in fs::read_dir(&current)
            .map_err(|error| format!("cannot read {}: {error}", current.display()))?
        {
            let path = entry
                .map_err(|error| format!("cannot read an entry in {}: {error}", current.display()))?
                .path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "mjs") {
                let relative = path
                    .strip_prefix(directory)
                    .expect("program source must be below its fixture directory")
                    .to_owned();
                let source = fs::read_to_string(&path)
                    .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
                sources.push((relative, source));
            }
        }
    }

    sources.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(sources)
}

fn execute_javascript(source: &str, mode: &str) -> Result<Value, String> {
    let temporary = tempdir().map_err(|error| format!("cannot create temporary dir: {error}"))?;
    execute_host(temporary.path(), json!({ "source": source, "mode": mode }))
}

fn execute_module_program(sources: &[(PathBuf, String)]) -> Result<Value, String> {
    let temporary = tempdir().map_err(|error| format!("cannot create temporary dir: {error}"))?;
    let program_directory = temporary.path().join("program");
    for (relative_path, source) in sources {
        let path = program_directory.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }
        fs::write(&path, source)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    }

    execute_host(
        temporary.path(),
        json!({ "entryPath": program_directory.join("entry.mjs") }),
    )
}

fn execute_host(temporary: &Path, request: Value) -> Result<Value, String> {
    let request_path = temporary.join("request.json");
    let result_path = temporary.join("result.json");
    let stderr_path = temporary.join("stderr.log");
    fs::write(&request_path, request.to_string())
        .map_err(|error| format!("cannot write host request: {error}"))?;
    let stderr = File::create(&stderr_path)
        .map_err(|error| format!("cannot create host stderr file: {error}"))?;
    let host = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/runtime_host.mjs");
    let mut child = Command::new("node")
        .arg(host)
        .arg(&request_path)
        .arg(&result_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|error| format!("cannot start Node.js: {error}"))?;

    let status = child
        .wait_timeout(EXECUTION_TIMEOUT)
        .map_err(|error| format!("cannot wait for Node.js: {error}"))?;
    let Some(status) = status else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!(
            "Node.js exceeded the {} second timeout",
            EXECUTION_TIMEOUT.as_secs(),
        ));
    };

    if !status.success() {
        let stderr = fs::read_to_string(&stderr_path).unwrap_or_default();
        return Err(format!("Node.js host failed: {stderr}"));
    }

    let result = fs::read(&result_path)
        .map_err(|error| format!("Node.js host did not produce a result: {error}"))?;
    serde_json::from_slice(&result).map_err(|error| format!("invalid host result: {error}"))
}

fn format_execution(execution: &Value) -> String {
    serde_json::to_string_pretty(execution).expect("host results must be valid JSON")
}
