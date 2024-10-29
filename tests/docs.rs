use fhir_utils::fhirrs::dispatch::RESOURCE_KEYS;
use fhir_utils::TaskRegistry;
use std::fs;
use std::path::PathBuf;

fn readme() -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("README.adoc")).unwrap()
}

fn first_column_codes(content: &str, section: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut inside = false;
    for line in content.lines() {
        if line.starts_with("== ") {
            inside = line.trim() == section;
            continue;
        }
        if !inside || !line.starts_with("| `") {
            continue;
        }
        if let Some(name) = line.trim_start_matches("| `").split('`').next() {
            names.push(name.to_string());
        }
    }
    names.sort();
    names.dedup();
    names
}

#[test]
fn the_task_table_lists_every_registered_task() {
    let documented = first_column_codes(&readme(), "== Tasks");
    let registered = TaskRegistry::new().names();

    assert!(!documented.is_empty());
    assert_eq!(documented, registered);
}

#[test]
fn every_resource_key_is_documented() {
    let content = readme();
    for (key, _) in RESOURCE_KEYS {
        assert!(
            content.contains(&format!("`{key}`")),
            "resource key missing from the readme: {key}"
        );
    }
}

#[test]
fn every_command_and_flag_is_documented() {
    let content = readme();
    for item in [
        "validate",
        "convert",
        "-d",
        "-f",
        "-c",
        "-o",
        "--strict",
        "RUST_LOG",
        "CSV_BUFFER_SIZE",
        "MAPPING_CONFIG_DIRECTORY",
        "MAPPING_CONFIG_FILE_NAME",
    ] {
        assert!(content.contains(item), "readme is missing: {item}");
    }
}
