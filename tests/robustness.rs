use fhir_utils::cli::{run_convert, ConvertRequest};
use serde_json::Value;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

const CONTRACT: &str = r#"{
    "general": {"timeZone": "UTC", "tenantId": "t1", "streamType": "live"},
    "fileDefinitions": {
        "patient": {"fileType": "csv", "resourceType": "Patient", "groupByKey": "patientInternalId"}
    }
}"#;

fn tree(input: &[u8]) -> TempDir {
    let base = TempDir::new().unwrap();
    fs::create_dir_all(base.path().join("input")).unwrap();
    fs::create_dir_all(base.path().join("config")).unwrap();
    fs::write(base.path().join("config/data-contract.json"), CONTRACT).unwrap();
    fs::write(base.path().join("input/patient.csv"), input).unwrap();
    base
}

fn convert(base: &Path, output: &Path) -> fhir_utils::cli::ConvertSummary {
    run_convert(&ConvertRequest::directory(
        base.to_path_buf(),
        output.to_path_buf(),
    ))
    .unwrap()
}

fn read(output: &Path, relative: &str) -> Value {
    serde_json::from_str(&fs::read_to_string(output.join(relative)).unwrap()).unwrap()
}

#[test]
fn byte_order_marks_carriage_returns_and_ragged_rows_convert() {
    let mut input = vec![0xef, 0xbb, 0xbf];
    input.extend_from_slice(
        b"patientInternalId,mrn,nameLast,gender\r\np1,12345,Smith,female\r\np2,999\r\np3,1,Jones,male,extra\r\n",
    );
    let base = tree(&input);
    let output = TempDir::new().unwrap();

    let summary = convert(base.path(), output.path());
    assert_eq!(summary.resources, 3);

    let first = read(output.path(), "p1/p1-Patient-patient-00001.json");
    assert_eq!(first["id"], "p1");
    assert_eq!(first["gender"], "female");
    assert_eq!(first["name"][0]["family"], "Smith");

    let short_row = read(output.path(), "p2/p2-Patient-patient-00001.json");
    assert_eq!(short_row["id"], "p2");
    assert!(short_row.get("gender").is_none());

    let long_row = read(output.path(), "p3/p3-Patient-patient-00001.json");
    assert_eq!(long_row["name"][0]["family"], "Jones");
}

#[test]
fn quoted_fields_survive_delimiters_and_newlines() {
    let base =
        tree(b"patientInternalId,nameLast,addressText\np1,\"Smith, Jr\",\"1 Main St\nApt 2\"\n");
    let output = TempDir::new().unwrap();

    let summary = convert(base.path(), output.path());
    assert_eq!(summary.resources, 1);

    let patient = read(output.path(), "p1/p1-Patient-patient-00001.json");
    assert_eq!(patient["name"][0]["family"], "Smith, Jr");
    assert_eq!(patient["address"][0]["text"], "1 Main St\nApt 2");
}

#[test]
fn header_only_input_produces_no_resources() {
    let base = tree(b"patientInternalId,nameLast\n");
    let output = TempDir::new().unwrap();

    let summary = convert(base.path(), output.path());
    assert_eq!(summary.files, 1);
    assert_eq!(summary.resources, 0);
}

#[test]
fn rows_without_a_group_key_use_the_default_bucket() {
    let base = tree(b"patientInternalId,nameLast\n,Solo\n");
    let output = TempDir::new().unwrap();

    convert(base.path(), output.path());
    let patient = read(
        output.path(),
        "NoGroupByKey/NoGroupByKey-Patient-patient-00001.json",
    );
    assert_eq!(patient["name"][0]["family"], "Solo");
    assert_eq!(patient["id"], "NoGroupByKey");
}

#[test]
fn a_latin_one_input_names_the_file_and_the_re_encoding_fix() {
    let base = tree(b"mrn,nameLast\n123,Beno\xeet\n");
    let output = base.path().join("out");
    let request = ConvertRequest::directory(base.path().to_path_buf(), output);
    let message = match run_convert(&request) {
        Ok(summary) => panic!("expected a failure, got {summary:?}"),
        Err(error) => error.to_string(),
    };

    assert!(message.contains("not valid UTF-8"), "{message}");
    assert!(message.contains("iconv"), "{message}");
    assert!(message.contains("patient.csv"), "{message}");
}

#[test]
fn rows_that_build_no_resource_are_counted_and_named() {
    let base = TempDir::new().unwrap();
    fs::create_dir_all(base.path().join("input")).unwrap();
    fs::create_dir_all(base.path().join("config")).unwrap();
    fs::write(
        base.path().join("config/data-contract.json"),
        r#"{"general": {"timeZone": "UTC", "tenantId": "t1", "streamType": "live"},
            "fileDefinitions": {"labs": {"fileType": "csv", "resourceType": "Observation",
                "groupByKey": "mrn"}}}"#,
    )
    .unwrap();
    fs::write(
        base.path().join("input/labs.csv"),
        "mrn,observationCode\n111,2160-0\n222,\n333,\n",
    )
    .unwrap();

    let output = base.path().join("out");
    let summary = convert(base.path(), &output);
    assert_eq!(summary.rows, 3);
    assert_eq!(summary.resources, 1);
    assert_eq!(summary.empty_rows, 2);

    let strict = run_convert(
        &ConvertRequest::directory(base.path().to_path_buf(), base.path().join("strict")).strict(),
    );
    let message = match strict {
        Ok(summary) => panic!("expected a failure, got {summary:?}"),
        Err(error) => error.to_string(),
    };
    assert!(message.contains("2 of 3 rows"), "{message}");
    assert!(message.contains("rows 2, 3"), "{message}");
}
