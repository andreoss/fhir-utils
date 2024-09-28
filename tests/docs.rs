use fhir_utils::fhirrs::dispatch::RESOURCE_KEYS;
use fhir_utils::TaskRegistry;
use std::fs;
use std::path::PathBuf;

fn doc(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("doc")
        .join(name);
    fs::read_to_string(path).unwrap()
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
fn the_task_reference_lists_every_registered_task() {
    let documented = first_column_codes(&doc("Tasks.adoc"), "== Tasks");
    let registered = TaskRegistry::new().names();

    assert!(!documented.is_empty());
    assert_eq!(documented, registered);
}

#[test]
fn the_contract_reference_lists_every_resource_key() {
    let content = doc("DataContract.adoc");
    for (key, _) in RESOURCE_KEYS {
        assert!(
            content.contains(&format!("`{key}`")),
            "resource key missing from the contract reference: {key}"
        );
    }
}

#[test]
fn the_cli_reference_lists_every_command() {
    let content = doc("Cli.adoc");
    for command in ["validate", "convert", "-d", "-f", "-c", "-o"] {
        assert!(
            content.contains(command),
            "cli reference is missing: {command}"
        );
    }
}
