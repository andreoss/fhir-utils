use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/contracts")
        .join(name)
}

fn validate(name: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_fhir-utils"))
        .args(["validate", "-f"])
        .arg(fixture(name))
        .output()
        .unwrap()
}

#[test]
fn validate_accepts_a_valid_contract() {
    let output = validate("valid.json");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("valid"));
}

#[test]
fn validate_rejects_an_invalid_timezone() {
    let output = validate("invalid_timezone.json");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("timezone"));
}

#[test]
fn validate_rejects_an_unknown_task() {
    let output = validate("unknown_task.json");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown task"));
}

fn fixture_tree() -> tempfile::TempDir {
    let base = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(base.path().join("input")).unwrap();
    std::fs::create_dir_all(base.path().join("config")).unwrap();
    std::fs::create_dir_all(base.path().join("out")).unwrap();
    std::fs::write(
        base.path().join("config/data-contract.json"),
        r#"{
            "general": {"timeZone": "UTC", "tenantId": "t1", "streamType": "live"},
            "fileDefinitions": {
                "patient": {"fileType": "csv", "resourceType": "Patient", "groupByKey": "patientInternalId"}
            }
        }"#,
    )
    .unwrap();
    std::fs::write(
        base.path().join("input/patient.csv"),
        "patientInternalId,nameLast\np1,Smith\n",
    )
    .unwrap();
    base
}

#[test]
fn convert_runs_in_directory_mode() {
    let base = fixture_tree();
    let output = base.path().join("out");
    let result = Command::new(env!("CARGO_BIN_EXE_fhir-utils"))
        .arg("convert")
        .arg("-d")
        .arg(base.path())
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.join("p1/p1-Patient-patient-00001.json").is_file());
}

#[test]
fn convert_runs_in_file_mode() {
    let base = fixture_tree();
    let output = base.path().join("out");
    let result = Command::new(env!("CARGO_BIN_EXE_fhir-utils"))
        .arg("convert")
        .arg("-f")
        .arg(base.path().join("input/patient.csv"))
        .arg("-c")
        .arg(base.path().join("config"))
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("wrote 1 resource"));
}

#[test]
fn convert_strict_mode_reports_an_ungroupable_file() {
    let base = fixture_tree();
    let contract = base.path().join("config/data-contract.json");
    let text = std::fs::read_to_string(&contract)
        .unwrap()
        .replace("\"patientInternalId\"}", "\"mrn\"}");
    std::fs::write(&contract, text).unwrap();

    let lenient = Command::new(env!("CARGO_BIN_EXE_fhir-utils"))
        .arg("convert")
        .arg("-d")
        .arg(base.path())
        .arg("-o")
        .arg(base.path().join("out"))
        .output()
        .unwrap();
    assert!(lenient.status.success());
    assert!(String::from_utf8_lossy(&lenient.stderr).contains("group key"));

    let strict = Command::new(env!("CARGO_BIN_EXE_fhir-utils"))
        .arg("convert")
        .arg("-d")
        .arg(base.path())
        .arg("-o")
        .arg(base.path().join("out-strict"))
        .arg("--strict")
        .output()
        .unwrap();
    assert!(!strict.status.success());
    assert!(String::from_utf8_lossy(&strict.stderr).contains("mrn"));
}

#[test]
fn convert_names_skipped_files_and_warns_about_stale_output() {
    let base = fixture_tree();
    std::fs::write(base.path().join("input/billing.csv"), "a,b\n1,2\n").unwrap();
    let output = base.path().join("out");

    let first = Command::new(env!("CARGO_BIN_EXE_fhir-utils"))
        .arg("convert")
        .arg("-d")
        .arg(base.path())
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(first.status.success());
    let summary = String::from_utf8_lossy(&first.stdout);
    assert!(
        summary.contains("skipped 1 file(s): billing.csv"),
        "{summary}"
    );
    assert!(!String::from_utf8_lossy(&first.stderr).contains("not empty"));

    let second = Command::new(env!("CARGO_BIN_EXE_fhir-utils"))
        .arg("convert")
        .arg("-d")
        .arg(base.path())
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(second.status.success());
    assert!(
        !String::from_utf8_lossy(&second.stderr).contains("not empty"),
        "a repeated run into the same directory must stay quiet"
    );

    let verbose = Command::new(env!("CARGO_BIN_EXE_fhir-utils"))
        .arg("convert")
        .arg("-d")
        .arg(base.path())
        .arg("-o")
        .arg(&output)
        .env("RUST_LOG", "info")
        .output()
        .unwrap();
    let logs = String::from_utf8_lossy(&verbose.stderr);
    assert!(logs.contains("output directory is not empty"), "{logs}");
    assert!(
        logs.contains("converted") && logs.contains("rows=1"),
        "{logs}"
    );
    assert!(logs.contains("no file definition matched"), "{logs}");
}

#[test]
fn convert_requires_an_input_selector() {
    let base = fixture_tree();
    let result = Command::new(env!("CARGO_BIN_EXE_fhir-utils"))
        .arg("convert")
        .arg("-o")
        .arg(base.path().join("out"))
        .output()
        .unwrap();
    assert!(!result.status.success());
}

#[test]
fn convert_file_mode_requires_a_config_directory() {
    let base = fixture_tree();
    let result = Command::new(env!("CARGO_BIN_EXE_fhir-utils"))
        .arg("convert")
        .arg("-f")
        .arg(base.path().join("input/patient.csv"))
        .arg("-o")
        .arg(base.path().join("out"))
        .output()
        .unwrap();
    assert!(!result.status.success());
}

#[test]
fn validate_resolves_external_definitions_next_to_the_contract() {
    let base = tempfile::TempDir::new().unwrap();
    std::fs::write(
        base.path().join("patient-definition.json"),
        r#"{"fileType": "csv", "resourceType": "Patient", "groupByKey": "mrn"}"#,
    )
    .unwrap();
    std::fs::write(
        base.path().join("data-contract.json"),
        r#"{"general": {"timeZone": "UTC", "tenantId": "t1", "streamType": "live"},
            "fileDefinitions": {"patient": "patient-definition.json"}}"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_fhir-utils"))
        .args(["validate", "-f"])
        .arg(base.path().join("data-contract.json"))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn validate_reports_a_missing_file() {
    let output = Command::new(env!("CARGO_BIN_EXE_fhir-utils"))
        .args(["validate", "-f", "no-such-contract.json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!output.stderr.is_empty());
}

#[test]
fn a_task_target_no_resource_reads_is_reported() {
    let base = fixture_tree();
    let contract = base.path().join("config/data-contract.json");
    std::fs::write(
        &contract,
        r#"{
            "general": {"timeZone": "UTC", "tenantId": "t1", "streamType": "live"},
            "fileDefinitions": {
                "patient": {"fileType": "csv", "resourceType": "Patient",
                    "groupByKey": "patientInternalId",
                    "tasks": [{"task": "rename_columns", "column_map": {"nameLast": "nameLastt"}}]}
            }
        }"#,
    )
    .unwrap();

    let validated = Command::new(env!("CARGO_BIN_EXE_fhir-utils"))
        .args(["validate", "-f"])
        .arg(&contract)
        .output()
        .unwrap();
    assert!(validated.status.success());
    let notes = String::from_utf8_lossy(&validated.stderr);
    assert!(notes.contains("nameLastt"), "{notes}");

    let strict = Command::new(env!("CARGO_BIN_EXE_fhir-utils"))
        .arg("convert")
        .arg("-d")
        .arg(base.path())
        .arg("-o")
        .arg(base.path().join("out"))
        .arg("--strict")
        .output()
        .unwrap();
    assert!(!strict.status.success());
    assert!(String::from_utf8_lossy(&strict.stderr).contains("nameLastt"));
}
