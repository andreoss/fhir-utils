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
fn validate_reports_a_missing_file() {
    let output = Command::new(env!("CARGO_BIN_EXE_fhir-utils"))
        .args(["validate", "-f", "no-such-contract.json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!output.stderr.is_empty());
}
