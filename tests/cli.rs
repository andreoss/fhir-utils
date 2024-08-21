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

#[test]
fn validate_reports_a_missing_file() {
    let output = Command::new(env!("CARGO_BIN_EXE_fhir-utils"))
        .args(["validate", "-f", "no-such-contract.json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!output.stderr.is_empty());
}
