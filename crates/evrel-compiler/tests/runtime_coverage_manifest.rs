use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde_json::{Map, Value};

const MANIFEST_PATH: &str = "tests/fixtures/runtime/coverage-manifest.json";
const ALLOWED_STATUSES: &[&str] = &[
    "pass",
    "partial",
    "xfail",
    "unsupported",
    "test262",
    "host",
    "n_a",
    "compiler_unit",
];

#[test]
fn semantic_coverage_manifest_matches_the_compiler_architecture() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(crate_root.join(MANIFEST_PATH)).expect("read coverage manifest"),
    )
    .expect("parse coverage manifest");

    assert_eq!(manifest["target"], "ECMA-262 2026");

    let evidence = object(&manifest, "evidence");
    validate_evidence(&crate_root, evidence);

    let frontend_expressions = object(&manifest, "frontend_expressions");
    let frontend_statements = object(&manifest, "frontend_statements");
    let ir_variants = object(&manifest, "ir_variants");
    let spec_sections = object(&manifest, "spec_sections");

    let expected_expressions = extract_prefixed_variants(
        &fs::read_to_string(crate_root.join("../evrel-frontend/src/lower/expression/mod.rs"))
            .expect("read expression lowering"),
        "Expression::",
    );
    assert_exact_keys(
        "frontend expression variants",
        frontend_expressions,
        &expected_expressions,
    );

    let expected_statements = extract_prefixed_variants(
        &fs::read_to_string(crate_root.join("../evrel-frontend/src/lower/statement/mod.rs"))
            .expect("read statement lowering"),
        "Statement::",
    );
    assert_exact_keys(
        "frontend statement variants",
        frontend_statements,
        &expected_statements,
    );

    let expected_ir_variants = extract_public_enum_variants(&crate_root.join("../evrel-ir/src"));
    assert_exact_keys(
        "public operation enum variants",
        ir_variants,
        &expected_ir_variants,
    );

    let expected_spec_sections = expected_spec_sections();
    assert_exact_keys("ECMA-262 sections", spec_sections, &expected_spec_sections);

    let maps = [
        ("frontend_expressions", frontend_expressions),
        ("frontend_statements", frontend_statements),
        ("ir_variants", ir_variants),
        ("spec_sections", spec_sections),
    ];
    validate_evidence_references(evidence, &maps);
    validate_all_runtime_fixtures_are_owned(&crate_root, evidence);
}

fn object<'a>(manifest: &'a Value, key: &str) -> &'a Map<String, Value> {
    manifest[key]
        .as_object()
        .unwrap_or_else(|| panic!("coverage manifest `{key}` must be an object"))
}

fn validate_evidence(crate_root: &Path, evidence: &Map<String, Value>) {
    let allowed = ALLOWED_STATUSES.iter().copied().collect::<BTreeSet<_>>();

    for (id, entry) in evidence {
        let entry = entry
            .as_object()
            .unwrap_or_else(|| panic!("evidence `{id}` must be an object"));
        let status = entry["status"]
            .as_str()
            .unwrap_or_else(|| panic!("evidence `{id}` must have a string status"));
        assert!(
            allowed.contains(status),
            "evidence `{id}` has unknown status `{status}`"
        );

        let paths = entry.get("paths").and_then(Value::as_array);
        let note = entry.get("note").and_then(Value::as_str);
        assert!(
            paths.is_some_and(|paths| !paths.is_empty())
                || note.is_some_and(|note| !note.is_empty()),
            "evidence `{id}` needs at least one path or a non-empty note"
        );

        if let Some(paths) = paths {
            for path in paths {
                let path = path
                    .as_str()
                    .unwrap_or_else(|| panic!("evidence `{id}` paths must be strings"));
                assert!(
                    crate_root.join(path).exists(),
                    "evidence `{id}` references missing path `{path}`"
                );
            }
        }
    }
}

fn extract_prefixed_variants(source: &str, prefix: &str) -> BTreeSet<String> {
    source
        .match_indices(prefix)
        .map(|(index, _)| {
            source[index + prefix.len()..]
                .chars()
                .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                .collect::<String>()
        })
        .filter(|variant| !variant.is_empty())
        .collect()
}

fn extract_public_enum_variants(directory: &Path) -> BTreeSet<String> {
    let mut paths = Vec::new();
    collect_rust_files(directory, &mut paths);
    paths.sort();

    let mut variants = BTreeSet::new();
    for path in paths {
        let source = fs::read_to_string(&path).expect("read operation source");
        let mut current_enum: Option<String> = None;
        let mut depth = 0_i32;

        for line in source.lines() {
            if current_enum.is_none() {
                if let Some(declaration) = line.strip_prefix("pub enum ") {
                    let name = declaration
                        .chars()
                        .take_while(|character| {
                            character.is_ascii_alphanumeric() || *character == '_'
                        })
                        .collect::<String>();
                    assert!(
                        !name.is_empty(),
                        "invalid public enum declaration in {path:?}"
                    );
                    current_enum = Some(name);
                    depth = brace_delta(line);
                }
                continue;
            }

            if depth == 1 && line.starts_with("    ") && !line.starts_with("        ") {
                let trimmed = line.trim_start();
                if trimmed
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_alphabetic())
                {
                    let variant = trimmed
                        .chars()
                        .take_while(|character| {
                            character.is_ascii_alphanumeric() || *character == '_'
                        })
                        .collect::<String>();
                    if !variant.is_empty() {
                        variants.insert(format!("{}::{variant}", current_enum.as_ref().unwrap()));
                    }
                }
            }

            depth += brace_delta(line);
            if depth == 0 {
                current_enum = None;
            }
        }
    }

    variants
}

fn collect_rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("read Rust source directory") {
        let path = entry.expect("read Rust source entry").path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn brace_delta(line: &str) -> i32 {
    line.chars().fold(0, |depth, character| match character {
        '{' => depth + 1,
        '}' => depth - 1,
        _ => depth,
    })
}

fn expected_spec_sections() -> BTreeSet<String> {
    let sections = [
        "6",
        "7",
        "8",
        "9",
        "10",
        "11",
        "12",
        "13.1",
        "13.2.1",
        "13.2.2",
        "13.2.3",
        "13.2.4",
        "13.2.5",
        "13.2.6",
        "13.2.7",
        "13.2.8",
        "13.2.9",
        "13.3.1",
        "13.3.2",
        "13.3.3",
        "13.3.4",
        "13.3.5",
        "13.3.6",
        "13.3.7",
        "13.3.8",
        "13.3.9",
        "13.3.10",
        "13.3.11",
        "13.3.12",
        "13.4",
        "13.5",
        "13.6",
        "13.7",
        "13.8",
        "13.9",
        "13.10",
        "13.11",
        "13.12",
        "13.13",
        "13.14",
        "13.15",
        "13.16",
        "14.1",
        "14.2",
        "14.3.1",
        "14.3.2",
        "14.3.3",
        "14.4",
        "14.5",
        "14.6",
        "14.7.1",
        "14.7.2",
        "14.7.3",
        "14.7.4",
        "14.7.5",
        "14.8",
        "14.9",
        "14.10",
        "14.11",
        "14.12",
        "14.13",
        "14.14",
        "14.15",
        "14.16",
        "15.1",
        "15.2",
        "15.3",
        "15.4",
        "15.5",
        "15.6",
        "15.7",
        "15.8",
        "15.9",
        "15.10",
        "16.1",
        "16.2.1",
        "16.2.2",
        "16.2.3",
        "17",
        "18",
        "19",
        "20",
        "21",
        "22",
        "23",
        "24",
        "25",
        "26",
        "27",
        "28",
        "29",
        "Annex A",
        "Annex B.1",
        "Annex B.2",
        "Annex B.3",
        "Annex C",
        "Annex D-F",
    ];
    sections.into_iter().map(String::from).collect()
}

fn assert_exact_keys(label: &str, actual: &Map<String, Value>, expected: &BTreeSet<String>) {
    let actual = actual.keys().cloned().collect::<BTreeSet<_>>();
    let missing = expected.difference(&actual).cloned().collect::<Vec<_>>();
    let stale = actual.difference(expected).cloned().collect::<Vec<_>>();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "{label} coverage is out of sync; missing={missing:?}, stale={stale:?}"
    );
}

fn validate_evidence_references(
    evidence: &Map<String, Value>,
    maps: &[(&str, &Map<String, Value>)],
) {
    let evidence_ids = evidence.keys().cloned().collect::<BTreeSet<_>>();
    let mut references = BTreeMap::<String, Vec<String>>::new();

    for (map_name, map) in maps {
        for (key, value) in *map {
            let evidence_id = value.as_str().unwrap_or_else(|| {
                panic!("coverage mapping `{map_name}.{key}` must name one evidence entry")
            });
            assert!(
                evidence_ids.contains(evidence_id),
                "coverage mapping `{map_name}.{key}` references missing evidence `{evidence_id}`"
            );
            references
                .entry(evidence_id.to_owned())
                .or_default()
                .push(format!("{map_name}.{key}"));
        }
    }

    let unused = evidence_ids
        .difference(&references.keys().cloned().collect())
        .cloned()
        .collect::<Vec<_>>();
    assert!(unused.is_empty(), "unused evidence entries: {unused:?}");
}

fn validate_all_runtime_fixtures_are_owned(crate_root: &Path, evidence: &Map<String, Value>) {
    let evidence_paths = evidence
        .values()
        .filter_map(Value::as_object)
        .filter_map(|entry| entry.get("paths"))
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(Value::as_str)
        .map(|path| crate_root.join(path))
        .collect::<Vec<_>>();

    let fixture_roots = [
        "tests/fixtures/runtime",
        "tests/fixtures/runtime-programs",
        "tests/fixtures/runtime-programs-known-failures",
    ];
    let mut fixtures = Vec::new();
    for root in fixture_roots {
        collect_javascript_files(&crate_root.join(root), &mut fixtures);
    }

    let unowned = fixtures
        .into_iter()
        .filter(|fixture| {
            !evidence_paths
                .iter()
                .any(|owner| fixture.starts_with(owner))
        })
        .map(|fixture| {
            fixture
                .strip_prefix(crate_root)
                .unwrap_or(&fixture)
                .display()
                .to_string()
        })
        .collect::<Vec<_>>();
    assert!(
        unowned.is_empty(),
        "runtime fixtures without evidence ownership: {unowned:?}"
    );
}

fn collect_javascript_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("read runtime fixture directory") {
        let path = entry.expect("read runtime fixture entry").path();
        if path.is_dir() {
            collect_javascript_files(&path, files);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "js" || extension == "mjs")
        {
            files.push(path);
        }
    }
}
