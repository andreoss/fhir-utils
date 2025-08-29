use fhir_utils::cli::{run_convert, ConvertRequest};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const GENERATED_ID: &str = "<generated>";
const GENERATED_TIMESTAMP: &str = "<timestamp>";

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/live")
}

fn convert_fixture_tree(output: &Path) -> fhir_utils::cli::ConvertSummary {
    run_convert(&ConvertRequest::directory(fixtures(), output.to_path_buf()))
        .expect("conversion failed")
}

fn is_generated_id(value: &str) -> bool {
    match value.split_once('.') {
        Some((nanos, hex)) => {
            !nanos.is_empty()
                && nanos.chars().all(|c| c.is_ascii_digit())
                && hex.len() == 32
                && hex.chars().all(|c| c.is_ascii_hexdigit())
        }
        None => false,
    }
}

fn normalize(value: &Value) -> Value {
    match value {
        Value::Object(entries) => {
            let mut normalized = serde_json::Map::new();
            for (key, entry) in entries {
                let entry = match (key.as_str(), entry) {
                    ("valueDateTime", Value::String(_)) => {
                        Value::String(GENERATED_TIMESTAMP.into())
                    }
                    (_, Value::String(text)) if is_generated_id(text) => {
                        Value::String(GENERATED_ID.into())
                    }
                    (_, entry) => normalize(entry),
                };
                normalized.insert(key.clone(), entry);
            }
            Value::Object(normalized)
        }
        Value::Array(items) => Value::Array(items.iter().map(normalize).collect()),
        Value::String(text) if is_generated_id(text) => Value::String(GENERATED_ID.into()),
        other => other.clone(),
    }
}

fn collect(root: &Path) -> BTreeMap<String, Value> {
    let mut collected = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .to_string();
            let content = fs::read_to_string(&path).unwrap();
            let value: Value = serde_json::from_str(&content).unwrap();
            collected.insert(relative, normalize(&value));
        }
    }
    collected
}

#[test]
fn live_extract_matches_the_golden_files() {
    let output = tempfile::TempDir::new().unwrap();
    let summary = convert_fixture_tree(output.path());
    assert_eq!(summary.files, 4);
    assert_eq!(summary.skipped, 1);

    let produced = collect(output.path());
    let expected_root = fixtures().join("expected");

    if std::env::var("UPDATE_GOLDEN").is_ok() {
        let _ = fs::remove_dir_all(&expected_root);
        for (name, value) in &produced {
            let path = expected_root.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(
                &path,
                format!("{}\n", serde_json::to_string_pretty(value).unwrap()),
            )
            .unwrap();
        }
    }

    let expected = collect(&expected_root);
    let produced_names: Vec<&String> = produced.keys().collect();
    let expected_names: Vec<&String> = expected.keys().collect();
    assert_eq!(produced_names, expected_names, "output file set changed");

    for (name, value) in &produced {
        assert_eq!(value, &expected[name], "resource changed: {name}");
    }
}

#[test]
fn live_extract_covers_every_input_shape() {
    let output = tempfile::TempDir::new().unwrap();
    convert_fixture_tree(output.path());
    let produced = collect(output.path());

    let types: Vec<&str> = produced
        .values()
        .filter_map(|value| value["resourceType"].as_str())
        .collect();
    assert!(types.contains(&"Patient"));
    assert!(types.contains(&"Encounter"));
    assert!(types.contains(&"Observation"));

    let patient = produced
        .values()
        .find(|value| value["resourceType"] == "Patient" && value["id"] == "p1")
        .unwrap();
    assert_eq!(patient["gender"], "female");
    assert_eq!(patient["name"][0]["family"], "SMITH");
    assert_eq!(patient["birthDate"], "1980-01-02");

    let invalid_mrn = produced
        .values()
        .find(|value| value["resourceType"] == "Patient" && value["id"] == "p2")
        .unwrap();
    let mrn = invalid_mrn["identifier"]
        .as_array()
        .unwrap()
        .iter()
        .find(|identifier| identifier["type"]["coding"][0]["code"] == "MR")
        .unwrap();
    assert_eq!(mrn["value"], "unknown");
    assert!(invalid_mrn["identifier"]
        .as_array()
        .unwrap()
        .iter()
        .all(|identifier| identifier["type"]["coding"][0]["code"] != "SS"));

    let encounter = produced
        .values()
        .find(|value| value["resourceType"] == "Encounter")
        .unwrap();
    assert_eq!(encounter["period"]["start"], "2021-06-01T08:00:00-04:00");

    let observation = produced
        .values()
        .find(|value| value["resourceType"] == "Observation")
        .unwrap();
    assert_eq!(observation["valueQuantity"]["unit"], "mg/dL");
    assert_eq!(
        observation["effectiveDateTime"],
        "2021-06-01T00:00:00-04:00"
    );

    let last_name_only = produced
        .values()
        .find(|value| value["resourceType"] == "Patient" && value["id"] == "p3")
        .unwrap();
    assert_eq!(last_name_only["name"][0]["family"], "Solo");
}

#[test]
fn regex_file_matching_selects_definitions() {
    let output = tempfile::TempDir::new().unwrap();
    let summary = run_convert(&ConvertRequest::single_file(
        fixtures().join("input/patient.csv"),
        fixtures().join("config-regex"),
        output.path().to_path_buf(),
    ))
    .unwrap();
    assert_eq!(summary.files, 1);
    assert_eq!(summary.resources, 2);
}

#[test]
fn invalid_contracts_are_rejected() {
    let output = tempfile::TempDir::new().unwrap();
    let result = run_convert(&ConvertRequest::single_file(
        fixtures().join("input/patient.csv"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/contracts"),
        output.path().to_path_buf(),
    ));
    assert!(result.is_err());
}
