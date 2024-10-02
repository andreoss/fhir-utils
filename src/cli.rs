use crate::config::config;
use crate::contract::Contract;
use crate::converter::{convert, ConversionOptions};
use crate::error::Error;
use crate::fhirutils::constants;
use crate::lookup::lookup_file_definition;
use crate::opener::{self, LocalOpener, Opener};
use crate::tasks::TaskRegistry;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::info;

#[derive(Clone, Default)]
pub struct ConvertRequest {
    pub base: Option<PathBuf>,
    pub file: Option<PathBuf>,
    pub config_dir: Option<PathBuf>,
    pub output: PathBuf,
    pub opener: Option<Arc<dyn Opener>>,
}

impl std::fmt::Debug for ConvertRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConvertRequest")
            .field("base", &self.base)
            .field("file", &self.file)
            .field("config_dir", &self.config_dir)
            .field("output", &self.output)
            .field("opener", &self.opener.is_some())
            .finish()
    }
}

impl ConvertRequest {
    pub fn directory(base: impl Into<PathBuf>, output: impl Into<PathBuf>) -> Self {
        Self {
            base: Some(base.into()),
            output: output.into(),
            ..Self::default()
        }
    }

    pub fn single_file(
        file: impl Into<PathBuf>,
        config_dir: impl Into<PathBuf>,
        output: impl Into<PathBuf>,
    ) -> Self {
        Self {
            file: Some(file.into()),
            config_dir: Some(config_dir.into()),
            output: output.into(),
            ..Self::default()
        }
    }

    pub fn with_opener(mut self, opener: Arc<dyn Opener>) -> Self {
        self.opener = Some(opener);
        self
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct ConvertSummary {
    pub files: usize,
    pub resources: usize,
    pub skipped: usize,
}

#[derive(Default)]
struct Counters {
    counts: HashMap<(String, String), usize>,
}

impl Counters {
    fn next(&mut self, group_by_key: &str, key: &str) -> usize {
        let entry = self
            .counts
            .entry((group_by_key.to_string(), key.to_string()))
            .or_insert(0);
        *entry += 1;
        *entry
    }
}

pub fn run_convert(request: &ConvertRequest) -> Result<ConvertSummary, Error> {
    let (inputs, config_dir) = resolve_inputs(request)?;
    let contract_path = config_dir.join(&config().mapping_config_file_name);
    if !contract_path.exists() {
        return Err(Error::Config(format!(
            "contract not found: {}",
            contract_path.display()
        )));
    }

    let source_opener = request
        .opener
        .clone()
        .unwrap_or_else(|| Arc::new(LocalOpener::new(config_dir.clone())));
    opener::set_opener(source_opener);
    let contract = Contract::load(&fs::read_to_string(&contract_path)?)?;
    let registry = TaskRegistry::new();
    fs::create_dir_all(&request.output)?;

    let mut counters = Counters::default();
    let mut summary = ConvertSummary::default();

    for input in inputs {
        let stem = file_stem(&input);
        let file_def = match lookup_file_definition(&contract, &stem) {
            Ok(file_def) => file_def,
            Err(_) => {
                info!(file = %input.display(), "no file definition matched");
                summary.skipped += 1;
                continue;
            }
        };

        let file_path = input.to_string_lossy().to_string();
        let options = ConversionOptions {
            contract: &contract,
            file_def,
            file_path: &file_path,
            registry: &registry,
            buffer_size: config().csv_buffer_size,
        };

        summary.files += 1;
        for row in convert(&input, options)? {
            if let Some(error) = row.exception {
                return Err(Error::Conversion(format!(
                    "row conversion failed for group key {} in {}: {error}",
                    row.group_by_key, file_path
                )));
            }
            for resource in row.resources {
                write_resource(&request.output, &row.group_by_key, &resource, &mut counters)?;
                summary.resources += 1;
            }
        }
    }

    Ok(summary)
}

fn resolve_inputs(request: &ConvertRequest) -> Result<(Vec<PathBuf>, PathBuf), Error> {
    if let Some(base) = &request.base {
        let input_dir = base.join("input");
        let config_dir = base.join("config");
        for path in [&input_dir, &config_dir] {
            if !path.is_dir() {
                return Err(Error::Config(format!("missing path: {}", path.display())));
            }
        }
        let mut inputs: Vec<PathBuf> = fs::read_dir(&input_dir)?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.is_file())
            .filter(|path| !file_name(path).starts_with('.'))
            .collect();
        inputs.sort();
        return Ok((inputs, config_dir));
    }

    let file = request
        .file
        .as_ref()
        .ok_or_else(|| Error::Config("convert needs -d or -f".into()))?;
    let config_dir = request
        .config_dir
        .as_ref()
        .ok_or_else(|| Error::Config("-f requires -c".into()))?;
    if !file.is_file() {
        return Err(Error::Config(format!("missing path: {}", file.display())));
    }
    if !config_dir.is_dir() {
        return Err(Error::Config(format!(
            "missing path: {}",
            config_dir.display()
        )));
    }
    Ok((vec![file.clone()], config_dir.clone()))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default()
}

pub fn safe_file_id(resource: &Value) -> String {
    let source_file_id = resource
        .get("meta")
        .and_then(|meta| meta.get("extension"))
        .and_then(|extensions| extensions.as_array())
        .and_then(|extensions| {
            extensions.iter().find(|extension| {
                extension
                    .get("url")
                    .and_then(|url| url.as_str())
                    .is_some_and(|url| {
                        url.contains(trailing_name(constants::EXT_META_SOURCE_FILE_ID))
                    })
            })
        })
        .and_then(|extension| extension.get("valueString"))
        .and_then(|value| value.as_str())
        .unwrap_or_default();

    let without_row = source_file_id.split(':').next().unwrap_or_default();
    let without_extension = without_row.split(".csv").next().unwrap_or(without_row);
    without_extension
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn trailing_name(url: &str) -> &str {
    url.rsplit(['/', ':']).next().unwrap_or(url)
}

fn write_resource(
    output: &Path,
    group_by_key: &str,
    resource: &Value,
    counters: &mut Counters,
) -> Result<PathBuf, Error> {
    let resource_type = resource
        .get("resourceType")
        .and_then(|value| value.as_str())
        .unwrap_or("Resource");
    let file_id = safe_file_id(resource);
    let count = counters.next(group_by_key, &format!("{resource_type}-{file_id}"));

    let directory = output.join(group_by_key);
    fs::create_dir_all(&directory)?;
    let path = directory.join(format!(
        "{group_by_key}-{resource_type}-{file_id}-{count:05}.json"
    ));
    fs::write(
        &path,
        format!("{}\n", serde_json::to_string_pretty(resource)?),
    )?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use crate::cli::*;
    use crate::fhirutils::constants;
    use serde_json::json;
    use serial_test::serial;
    use std::fs;
    use tempfile::TempDir;

    const CONTRACT: &str = r#"{
        "general": {
            "timeZone": "UTC",
            "tenantId": "tenant1",
            "streamType": "live",
            "assigningAuthority": "authority"
        },
        "fileDefinitions": {
            "patient": {
                "fileType": "csv",
                "resourceType": "Patient",
                "groupByKey": "patientInternalId"
            }
        }
    }"#;

    fn base_directory() -> TempDir {
        let base = TempDir::new().unwrap();
        fs::create_dir_all(base.path().join("input")).unwrap();
        fs::create_dir_all(base.path().join("config")).unwrap();
        fs::write(base.path().join("config/data-contract.json"), CONTRACT).unwrap();
        fs::write(
            base.path().join("input/patient.csv"),
            "patientInternalId,nameLast,nameFirst\np1,Smith,Ann\np2,Jones,Bob\n",
        )
        .unwrap();
        fs::write(base.path().join("input/unmatched.csv"), "a,b\n1,2\n").unwrap();
        base
    }

    #[test]
    fn safe_file_id_strips_the_row_suffix_and_extension() {
        let resource = json!({
            "meta": {"extension": [
                {"url": constants::EXT_META_SOURCE_FILE_ID, "valueString": "patient data.csv:00007"}
            ]}
        });
        assert_eq!(safe_file_id(&resource), "patient_data");
        assert_eq!(safe_file_id(&json!({})), "");
    }

    #[test]
    #[serial]
    fn directory_mode_writes_grouped_resources() {
        let base = base_directory();
        let output = TempDir::new().unwrap();
        let request =
            ConvertRequest::directory(base.path().to_path_buf(), output.path().to_path_buf());

        let summary = run_convert(&request).unwrap();
        assert_eq!(summary.files, 1);
        assert_eq!(summary.skipped, 1);
        assert_eq!(summary.resources, 2);

        let first = output.path().join("p1/p1-Patient-patient-00001.json");
        assert!(first.is_file(), "missing {}", first.display());
        assert!(output
            .path()
            .join("p2/p2-Patient-patient-00001.json")
            .is_file());

        let content = fs::read_to_string(&first).unwrap();
        assert!(content.contains("\"resourceType\": \"Patient\""));
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["name"][0]["family"], json!("Smith"));
        let keys: Vec<&str> = parsed
            .as_object()
            .unwrap()
            .keys()
            .map(|key| key.as_str())
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
    }

    #[test]
    #[serial]
    fn file_mode_needs_a_config_directory() {
        let base = base_directory();
        let output = TempDir::new().unwrap();
        let request = ConvertRequest {
            file: Some(base.path().join("input/patient.csv")),
            output: output.path().to_path_buf(),
            ..ConvertRequest::default()
        };
        assert!(run_convert(&request).is_err());

        let request = ConvertRequest::single_file(
            base.path().join("input/patient.csv"),
            base.path().join("config"),
            output.path().to_path_buf(),
        );
        let summary = run_convert(&request).unwrap();
        assert_eq!(summary.files, 1);
        assert_eq!(summary.resources, 2);
    }

    #[test]
    #[serial]
    fn unmatched_files_are_skipped() {
        let base = base_directory();
        let output = TempDir::new().unwrap();
        let request = ConvertRequest::single_file(
            base.path().join("input/unmatched.csv"),
            base.path().join("config"),
            output.path().to_path_buf(),
        );
        let summary = run_convert(&request).unwrap();
        assert_eq!(
            summary,
            ConvertSummary {
                files: 0,
                resources: 0,
                skipped: 1
            }
        );
    }

    #[test]
    #[serial]
    fn a_supplied_opener_serves_contract_relative_sources() {
        let base = base_directory();
        fs::write(
            base.path().join("config/data-contract.json"),
            CONTRACT.replace(
                r#""groupByKey": "patientInternalId""#,
                r#""groupByKey": "patientInternalId",
                "tasks": [{"task": "map_codes", "code_map": {"nameLast": "names.csv"}}]"#,
            ),
        )
        .unwrap();
        let output = TempDir::new().unwrap();

        let request = ConvertRequest::directory(base.path(), output.path()).with_opener(Arc::new(
            crate::opener::MemoryOpener::new(vec![(
                "names.csv".into(),
                b"source_value,target_value\nSmith,Renamed\n".to_vec(),
            )]),
        ));

        run_convert(&request).unwrap();
        let content =
            fs::read_to_string(output.path().join("p1/p1-Patient-patient-00001.json")).unwrap();
        let patient: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(patient["name"][0]["family"], json!("Renamed"));
    }

    #[test]
    #[serial]
    fn missing_directories_raise() {
        let output = TempDir::new().unwrap();
        let request =
            ConvertRequest::directory(output.path().join("nope"), output.path().to_path_buf());
        assert!(run_convert(&request).is_err());
    }

    #[test]
    #[serial]
    fn counters_increment_per_resource_type_and_source_file() {
        let base = base_directory();
        fs::write(
            base.path().join("input/patient.csv"),
            "patientInternalId,nameLast\np1,Smith\np1,Jones\n",
        )
        .unwrap();
        let output = TempDir::new().unwrap();
        let request =
            ConvertRequest::directory(base.path().to_path_buf(), output.path().to_path_buf());

        run_convert(&request).unwrap();
        assert!(output
            .path()
            .join("p1/p1-Patient-patient-00001.json")
            .is_file());
        assert!(output
            .path()
            .join("p1/p1-Patient-patient-00002.json")
            .is_file());
    }
}
