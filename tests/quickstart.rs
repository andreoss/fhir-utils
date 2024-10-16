use fhir_utils::cli::{run_convert, ConvertRequest};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/quickstart")
}

fn read(output: &TempDir, relative: &str) -> Value {
    let path = output.path().join(relative);
    assert!(path.is_file(), "missing {}", path.display());
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn the_quickstart_example_converts_as_documented() {
    let output = TempDir::new().unwrap();
    let summary = run_convert(
        &ConvertRequest::single_file(
            fixtures().join("input/patients.csv"),
            fixtures().join("config"),
            output.path().to_path_buf(),
        )
        .strict(),
    )
    .expect("the quickstart example must convert in strict mode");

    assert_eq!(summary.files, 1);
    assert_eq!(summary.resources, 2);
    assert_eq!(summary.skipped(), 0);

    let first = read(&output, "1234567/1234567-Patient-patients-00001.json");
    assert_eq!(first["resourceType"], "Patient");
    assert_eq!(first["id"], "1234567");
    assert_eq!(first["gender"], "female");
    assert_eq!(first["name"][0]["family"], "Smith");
    assert_eq!(first["birthDate"], "1980-01-02");

    let identifiers = first["identifier"].as_array().unwrap();
    let code_of = |code: &str| {
        identifiers
            .iter()
            .any(|identifier| identifier["type"]["coding"][0]["code"] == code)
    };
    assert!(code_of("MR"));
    assert!(code_of("SS"));

    let second = read(&output, "7654321/7654321-Patient-patients-00001.json");
    assert_eq!(second["gender"], "male");
    assert!(
        !second["identifier"]
            .as_array()
            .unwrap()
            .iter()
            .any(|identifier| identifier["type"]["coding"][0]["code"] == "SS"),
        "a blank ssn must not become an identifier"
    );
}

#[test]
fn the_quickstart_document_matches_its_fixtures() {
    let doc =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("doc/Quickstart.adoc"))
            .unwrap();
    let contract = fs::read_to_string(fixtures().join("config/data-contract.json")).unwrap();
    let extract = fs::read_to_string(fixtures().join("input/patients.csv")).unwrap();
    let codes = fs::read_to_string(fixtures().join("config/sex.csv")).unwrap();

    for line in contract.lines().chain(extract.lines()).chain(codes.lines()) {
        let line = line.trim_end();
        assert!(
            doc.contains(line),
            "the quickstart document does not show: {line}"
        );
    }
}
