use crate::config::config;
use crate::contract::Contract;
use crate::converter::{convert, ConversionOptions};
use crate::error::Error;
use crate::fhirutils::constants;
use crate::lookup::matched_definition;
use crate::opener::{self, LocalOpener, Opener};
use crate::tasks::TaskRegistry;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Clone, Default)]
pub struct ConvertRequest {
    pub base: Option<PathBuf>,
    pub file: Option<PathBuf>,
    pub config_dir: Option<PathBuf>,
    pub output: PathBuf,
    pub opener: Option<Arc<dyn Opener>>,
    pub strict: bool,
    pub bundle: Option<String>,
    pub verbose: bool,
    /// Fallback configuration directories; the primary directory shadows them.
    pub config_layers: Vec<PathBuf>,
    /// Expected SHA-256 of the contract bytes; a mismatch stops the run.
    pub contract_digest: Option<String>,
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
            .field("strict", &self.strict)
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

    pub fn strict(mut self) -> Self {
        self.strict = true;
        self
    }

    pub fn bundled(mut self, bundle_type: impl Into<String>) -> Self {
        self.bundle = Some(bundle_type.into());
        self
    }

    pub fn verbose(mut self) -> Self {
        self.verbose = true;
        self
    }

    pub fn with_config_layer(mut self, layer: impl Into<PathBuf>) -> Self {
        self.config_layers.push(layer.into());
        self
    }

    pub fn pinned(mut self, digest: impl Into<String>) -> Self {
        self.contract_digest = Some(digest.into());
        self
    }
}

const BUNDLE_TYPES: &[&str] = &["collection", "batch", "transaction"];

#[derive(Debug, Default, PartialEq)]
pub struct ConvertSummary {
    pub files: usize,
    pub rows: usize,
    pub resources: usize,
    pub empty_rows: usize,
    pub deduplicated: usize,
    pub skipped_files: Vec<String>,
}

impl ConvertSummary {
    pub fn skipped(&self) -> usize {
        self.skipped_files.len()
    }
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

/// Row numbers for a diagnostic, listing at most the first five.
fn row_numbers(rows: &[usize]) -> String {
    let shown: Vec<String> = rows.iter().take(5).map(usize::to_string).collect();
    let rest = rows.len().saturating_sub(shown.len());
    if rest > 0 {
        format!("rows {} and {rest} more", shown.join(", "))
    } else {
        format!("rows {}", shown.join(", "))
    }
}

/// Turns a row or reader failure into a message naming the file and the likely fix.
fn input_failure(file_path: &str, group_by_key: &str, error: Error) -> Error {
    if let Error::Csv(csv_error) = &error {
        if matches!(csv_error.kind(), csv::ErrorKind::Utf8 { .. }) {
            return Error::Config(format!(
                "{file_path} is not valid UTF-8: {csv_error}; re-encode it first, \
                 for example with iconv -f windows-1252 -t utf-8"
            ));
        }
    }
    match error {
        Error::Conversion(_) => error,
        other if group_by_key.is_empty() => {
            Error::Conversion(format!("{file_path} failed: {other}"))
        }
        other => Error::Conversion(format!("row {group_by_key} in {file_path} failed: {other}")),
    }
}

fn load_contract(request: &ConvertRequest, config_dir: &Path) -> Result<Contract, Error> {
    let mut layers = vec![config_dir.to_path_buf()];
    layers.extend(request.config_layers.iter().cloned());
    let name = &config().mapping_config_file_name;
    let contract_path = layers
        .iter()
        .map(|layer| layer.join(name))
        .find(|path| opener::scheme(&path.to_string_lossy()).is_some() || path.exists())
        .ok_or_else(|| {
            Error::Config(format!(
                "contract not found: {}",
                config_dir.join(name).display()
            ))
        })?;
    let remote_config = opener::scheme(&contract_path.to_string_lossy()).is_some();

    let source_opener = request.opener.clone().unwrap_or_else(|| {
        if layers.len() == 1 {
            Arc::new(LocalOpener::new(layers[0].clone()))
        } else {
            Arc::new(opener::LayeredOpener::new(
                layers
                    .iter()
                    .map(|layer| Arc::new(LocalOpener::new(layer.clone())) as Arc<dyn Opener>)
                    .collect(),
            ))
        }
    });
    opener::set_opener(source_opener);
    let contract_text = if remote_config {
        opener::read_to_string(&contract_path.to_string_lossy())?
    } else {
        fs::read_to_string(&contract_path)?
    };
    if let Some(expected) = &request.contract_digest {
        use sha2::Digest as _;
        let actual = format!("{:x}", sha2::Sha256::digest(contract_text.as_bytes()));
        if !actual.eq_ignore_ascii_case(expected.trim_start_matches("sha256:")) {
            return Err(Error::Config(format!(
                "contract digest mismatch: expected {expected}, found sha256:{actual}"
            )));
        }
    }
    let contract = Contract::load(&contract_text)?;
    let warnings = contract.lint();
    if request.strict && !warnings.is_empty() {
        return Err(Error::Config(warnings.join("; ")));
    }
    for warning in &warnings {
        warn!("{warning}");
    }
    Ok(contract)
}

pub fn run_transform(
    request: &ConvertRequest,
    out: &mut dyn std::io::Write,
) -> Result<ConvertSummary, Error> {
    let (inputs, config_dir) = resolve_inputs(request)?;
    let contract = load_contract(request, &config_dir)?;
    let registry = TaskRegistry::new();
    let mut summary = ConvertSummary::default();

    for input in inputs {
        let stem = file_stem(&input);
        let (_, file_def) = match matched_definition(&contract, &stem) {
            Ok(found) => found,
            Err(_) => {
                summary.skipped_files.push(file_name(&input));
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
            strict: request.strict,
        };

        summary.files += 1;
        let rows = crate::converter::transform(&input, options)
            .map_err(|error| input_failure(&file_path, "", error))?;
        for row in rows {
            summary.rows += 1;
            if let Some(error) = row.exception {
                return Err(input_failure(&file_path, &row.group_by_key, error));
            }
            writeln!(out, "{}", serde_json::to_string(&row.record)?)
                .map_err(|error| Error::Config(format!("cannot write a record: {error}")))?;
        }
    }
    Ok(summary)
}

pub fn run_convert(request: &ConvertRequest) -> Result<ConvertSummary, Error> {
    if let Some(bundle_type) = &request.bundle {
        if !BUNDLE_TYPES.contains(&bundle_type.as_str()) {
            return Err(Error::Config(format!(
                "unknown bundle type {bundle_type}; accepted: {}",
                BUNDLE_TYPES.join(", ")
            )));
        }
    }
    let (inputs, config_dir) = resolve_inputs(request)?;
    let contract = load_contract(request, &config_dir)?;
    let registry = TaskRegistry::new();
    let output = &request.output;
    let writer: Arc<dyn Opener> = if opener::scheme(&output.to_string_lossy()).is_some() {
        opener::opener()
    } else {
        Arc::new(LocalOpener::new("."))
    };
    if opener::scheme(&output.to_string_lossy()).is_none() {
        fs::create_dir_all(output).map_err(|error| {
            Error::Config(format!(
                "cannot create output directory {}: {error}",
                output.display()
            ))
        })?;
        let mut entries = fs::read_dir(output).map_err(|error| {
            Error::Config(format!(
                "cannot read output directory {}: {error}",
                output.display()
            ))
        })?;
        if entries.next().is_some() {
            info!(
                output = %request.output.display(),
                "output directory is not empty; files from earlier runs are left in place"
            );
        }
    }

    let mut counters = Counters::default();
    let mut summary = ConvertSummary::default();
    let mut bundles: std::collections::BTreeMap<String, Vec<Value>> =
        std::collections::BTreeMap::new();
    let mut bundle_index: HashMap<(String, String), (String, usize)> = HashMap::new();
    let mut written_paths: HashMap<(String, String), PathBuf> = HashMap::new();
    let mut seen_remote: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    let remote_output = opener::scheme(&output.to_string_lossy()).is_some();
    let mut all_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut all_references: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut hollow: Vec<String> = Vec::new();

    for input in inputs {
        let stem = file_stem(&input);
        let (matcher, file_def) = match matched_definition(&contract, &stem) {
            Ok(found) => found,
            Err(_) => {
                info!(file = %input.display(), "no file definition matched");
                summary.skipped_files.push(file_name(&input));
                continue;
            }
        };
        info!(
            file = %input.display(),
            definition = matcher,
            resource_type = file_def.resource_type,
            "file definition matched"
        );

        let file_path = input.to_string_lossy().to_string();
        let options = ConversionOptions {
            contract: &contract,
            file_def,
            file_path: &file_path,
            registry: &registry,
            buffer_size: config().csv_buffer_size,
            strict: request.strict,
        };

        summary.files += 1;
        let written = summary.resources;
        let mut rows = 0usize;
        let mut empty: Vec<usize> = Vec::new();
        let mut write_time = std::time::Duration::ZERO;
        let rows_iter =
            convert(&input, options).map_err(|error| input_failure(&file_path, "", error))?;
        for row in rows_iter {
            rows += 1;
            if let Some(error) = row.exception {
                return Err(input_failure(&file_path, &row.group_by_key, error));
            }
            if row.resources.is_empty() {
                empty.push(row.row_num);
            }
            for resource in row.resources {
                let resource_type = resource
                    .get("resourceType")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string();
                let id = resource
                    .get("id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string();
                let key = (resource_type, id);
                if !key.1.is_empty() {
                    all_ids.insert(format!("{}/{}", key.0, key.1));
                }
                collect_references(&resource, &mut all_references);
                if is_hollow(&resource) {
                    hollow.push(format!("{}/{}", key.0, key.1));
                }
                if !key.1.is_empty() {
                    if remote_output && request.bundle.is_none() {
                        if !seen_remote.insert(key.clone()) {
                            summary.deduplicated += 1;
                            continue;
                        }
                    } else if let Some((group, index)) = bundle_index.get(&key) {
                        let earlier = &mut bundles.get_mut(group).unwrap()[*index];
                        merge_duplicate(earlier, &resource, &key);
                        summary.deduplicated += 1;
                        continue;
                    } else if let Some(path) = written_paths.get(&key) {
                        let started = std::time::Instant::now();
                        merge_into_file(&*writer, path, &resource, &key)?;
                        write_time += started.elapsed();
                        summary.deduplicated += 1;
                        continue;
                    }
                }
                if request.bundle.is_some() {
                    let entries = bundles.entry(row.group_by_key.clone()).or_default();
                    if !key.1.is_empty() {
                        bundle_index.insert(key, (row.group_by_key.clone(), entries.len()));
                    }
                    entries.push(resource);
                } else {
                    let started = std::time::Instant::now();
                    let path = write_resource(
                        &*writer,
                        &request.output,
                        &row.group_by_key,
                        &resource,
                        &mut counters,
                    )?;
                    write_time += started.elapsed();
                    if !key.1.is_empty() && !remote_output {
                        written_paths.insert(key, path);
                    }
                }
                summary.resources += 1;
            }
        }
        summary.rows += rows;
        summary.empty_rows += empty.len();
        if !empty.is_empty() {
            let detail = row_numbers(&empty);
            if request.strict {
                return Err(Error::Conversion(format!(
                    "{} of {rows} rows in {file_path} produced no resource: {detail}",
                    empty.len()
                )));
            }
            warn!(
                file = %file_path,
                rows,
                empty = empty.len(),
                "rows produced no resource: {detail}"
            );
        }
        info!(
            file = %input.display(),
            rows,
            resources = summary.resources - written,
            "converted"
        );
        let phases = crate::converter::take_phase_timings();
        if request.verbose {
            eprintln!(
                "timings {}: read {:.1?}, tasks {:.1?}, build {:.1?}, write {:.1?}",
                input.display(),
                phases.read,
                phases.tasks,
                phases.build,
                write_time
            );
        }
    }

    if let Some(bundle_type) = &request.bundle {
        let started = std::time::Instant::now();
        for (group_by_key, resources) in bundles {
            write_bundle(
                &*writer,
                &request.output,
                &group_by_key,
                bundle_type,
                resources,
            )?;
        }
        if request.verbose {
            eprintln!("timings bundles: write {:.1?}", started.elapsed());
        }
    }

    let dangling: Vec<String> = all_references
        .into_iter()
        .filter(|reference| !all_ids.contains(reference))
        .collect();
    for (finding, values) in [
        ("dangling references", &dangling),
        ("hollow resources", &hollow),
    ] {
        if values.is_empty() {
            continue;
        }
        let shown: Vec<&str> = values.iter().take(5).map(String::as_str).collect();
        let rest = values.len().saturating_sub(shown.len());
        let more = if rest > 0 {
            format!(" and {rest} more")
        } else {
            String::new()
        };
        let detail = format!("{} {finding}: {}{more}", values.len(), shown.join(", "));
        if request.strict {
            return Err(Error::Conversion(detail));
        }
        warn!("{detail}");
    }

    Ok(summary)
}

/// Collects `Type/id` reference strings; URIs and fragments are not checked.
fn collect_references(value: &Value, references: &mut std::collections::BTreeSet<String>) {
    match value {
        Value::Object(entries) => {
            for (name, value) in entries {
                if name == "reference" {
                    if let Some(reference) = value.as_str() {
                        let local = reference.split_once('/').is_some_and(|(head, _)| {
                            !head.is_empty() && head.chars().all(|c| c.is_ascii_alphabetic())
                        });
                        if local {
                            references.insert(reference.to_string());
                        }
                        continue;
                    }
                }
                collect_references(value, references);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_references(item, references);
            }
        }
        _ => {}
    }
}

/// A resource carrying nothing beyond its type, id and meta.
fn is_hollow(resource: &Value) -> bool {
    match resource.as_object() {
        Some(entries) => entries
            .keys()
            .all(|key| matches!(key.as_str(), "resourceType" | "id" | "meta")),
        None => true,
    }
}

fn write_bundle(
    writer: &dyn Opener,
    output: &Path,
    group_by_key: &str,
    bundle_type: &str,
    resources: Vec<Value>,
) -> Result<PathBuf, Error> {
    let segment = safe_segment(group_by_key);
    let path = output.join(&segment).join(format!("{segment}-Bundle.json"));
    let bundle = build_bundle(bundle_type, resources);
    let write = |path: &Path| -> Result<(), Error> {
        let mut sink = writer.create(&path.to_string_lossy())?;
        sink.write_all(format!("{}\n", serde_json::to_string_pretty(&bundle)?).as_bytes())?;
        sink.flush()?;
        Ok(())
    };
    write(&path).map_err(|error| {
        Error::Config(format!(
            "cannot write {} for group key {group_by_key}: {error}",
            path.display()
        ))
    })?;
    Ok(path)
}

/// Deep-merges a duplicate into the resource seen earlier: objects merge,
/// arrays union, a conflicting scalar keeps the first value and warns.
fn merge_duplicate(base: &mut Value, incoming: &Value, key: &(String, String)) {
    if merge_values(base, incoming) {
        warn!(
            resource_type = %key.0,
            id = %key.1,
            "duplicate resource carries conflicting values; the first value wins"
        );
    }
}

fn merge_values(base: &mut Value, incoming: &Value) -> bool {
    match (base, incoming) {
        (Value::Object(base), Value::Object(incoming)) => {
            let mut conflict = false;
            for (name, value) in incoming {
                match base.get_mut(name) {
                    Some(existing) => conflict |= merge_values(existing, value),
                    None => {
                        base.insert(name.clone(), value.clone());
                    }
                }
            }
            conflict
        }
        (Value::Array(base), Value::Array(incoming)) => {
            for item in incoming {
                if !base.contains(item) {
                    base.push(item.clone());
                }
            }
            false
        }
        (base, incoming) => *base != *incoming,
    }
}

fn merge_into_file(
    writer: &dyn Opener,
    path: &Path,
    incoming: &Value,
    key: &(String, String),
) -> Result<(), Error> {
    let content = fs::read_to_string(path).map_err(|error| {
        Error::Config(format!(
            "cannot reread {} to merge a duplicate: {error}",
            path.display()
        ))
    })?;
    let mut base: Value = serde_json::from_str(&content)?;
    merge_duplicate(&mut base, incoming, key);
    let mut sink = writer.create(&path.to_string_lossy())?;
    sink.write_all(format!("{}\n", serde_json::to_string_pretty(&base)?).as_bytes())?;
    sink.flush()?;
    Ok(())
}

fn build_bundle(bundle_type: &str, resources: Vec<Value>) -> Value {
    let entries: Vec<Value> = resources
        .into_iter()
        .map(|resource| {
            let resource_type = resource
                .get("resourceType")
                .and_then(|value| value.as_str())
                .unwrap_or("Resource");
            let id = resource
                .get("id")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let reference = format!("{resource_type}/{id}");
            let mut entry = serde_json::Map::new();
            entry.insert("fullUrl".into(), Value::String(reference.clone()));
            if bundle_type != "collection" {
                entry.insert(
                    "request".into(),
                    serde_json::json!({"method": "PUT", "url": reference}),
                );
            }
            entry.insert("resource".into(), resource);
            Value::Object(entry)
        })
        .collect();
    serde_json::json!({
        "resourceType": "Bundle",
        "type": bundle_type,
        "id": uuid::Uuid::new_v4().to_string(),
        "meta": {
            "lastUpdated": chrono::Utc::now()
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        },
        "entry": entries
    })
}

fn resolve_inputs(request: &ConvertRequest) -> Result<(Vec<PathBuf>, PathBuf), Error> {
    if let Some(base) = &request.base {
        let input_dir = base.join("input");
        let config_dir = base.join("config");
        for path in [&input_dir, &config_dir] {
            if !path.is_dir() {
                return Err(Error::Config(format!(
                    "directory not found: {}",
                    path.display()
                )));
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
    let fallback = PathBuf::from(&config().mapping_config_directory);
    let config_dir = request.config_dir.as_ref().unwrap_or(&fallback);
    if !file.is_file() {
        return Err(Error::Config(format!(
            "input file not found: {}",
            file.display()
        )));
    }
    if !config_dir.is_dir() {
        return Err(Error::Config(format!(
            "directory not found: {}",
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

pub fn safe_segment(value: &str) -> String {
    let mapped: String = value
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    if mapped.is_empty() || mapped.chars().all(|c| c == '.') {
        "_".to_string()
    } else {
        mapped
    }
}

fn write_resource(
    writer: &dyn Opener,
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

    let segment = safe_segment(group_by_key);
    let path = output.join(&segment).join(format!(
        "{segment}-{resource_type}-{file_id}-{count:05}.json"
    ));
    let write = |path: &Path| -> Result<(), Error> {
        let mut sink = writer.create(&path.to_string_lossy())?;
        sink.write_all(format!("{}\n", serde_json::to_string_pretty(resource)?).as_bytes())?;
        sink.flush()?;
        Ok(())
    };
    write(&path).map_err(|error| {
        Error::Config(format!(
            "cannot write {} for group key {group_by_key}: {error}",
            path.display()
        ))
    })?;
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
        assert_eq!(summary.skipped(), 1);
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
        assert_eq!(summary.files, 0);
        assert_eq!(summary.resources, 0);
        assert_eq!(summary.skipped(), 1);
        assert_eq!(summary.skipped_files, vec!["unmatched.csv".to_string()]);
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
    fn config_layers_resolve_first_match_wins() {
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
        let layer = TempDir::new().unwrap();
        fs::write(
            layer.path().join("names.csv"),
            "source_value,target_value\nSmith,FromLayer\n",
        )
        .unwrap();
        fs::write(
            layer.path().join("data-contract.json"),
            r#"{"general": {}, "fileDefinitions": {}}"#,
        )
        .unwrap();
        let output = TempDir::new().unwrap();

        let request =
            ConvertRequest::directory(base.path(), output.path()).with_config_layer(layer.path());
        run_convert(&request).unwrap();

        let content =
            fs::read_to_string(output.path().join("p1/p1-Patient-patient-00001.json")).unwrap();
        let patient: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(
            patient["name"][0]["family"],
            json!("FromLayer"),
            "the map file resolves from the fallback layer, the contract from the primary"
        );
    }

    #[test]
    fn a_contract_digest_pins_the_contract_bytes() {
        use sha2::Digest as _;
        let base = base_directory();
        let contract_bytes = fs::read(base.path().join("config/data-contract.json")).unwrap();
        let digest = format!("{:x}", sha2::Sha256::digest(&contract_bytes));
        let output = TempDir::new().unwrap();

        let pinned = ConvertRequest::directory(base.path(), output.path()).pinned(digest.clone());
        assert!(run_convert(&pinned).is_ok());

        let wrong = ConvertRequest::directory(base.path(), output.path())
            .pinned("sha256:0000000000000000000000000000000000000000000000000000000000000000");
        let error = run_convert(&wrong).unwrap_err().to_string();
        assert!(error.contains("contract digest mismatch"), "{error}");
        assert!(error.contains(&digest), "{error}");
    }

    #[test]
    fn hollow_resources_are_detected() {
        assert!(is_hollow(
            &json!({"resourceType": "Patient", "id": "p1", "meta": {}})
        ));
        assert!(!is_hollow(
            &json!({"resourceType": "Patient", "id": "p1", "gender": "female"})
        ));
    }

    #[test]
    fn references_collect_only_local_type_id_forms() {
        let resource = json!({
            "subject": {"reference": "Patient/p1"},
            "performer": [{"reference": "urn:uuid:123"}],
            "attachment": {"url": "http://example.org/Binary/1"}
        });
        let mut references = std::collections::BTreeSet::new();
        collect_references(&resource, &mut references);
        assert_eq!(
            references.into_iter().collect::<Vec<_>>(),
            vec!["Patient/p1".to_string()]
        );
    }

    fn conditions_only_base() -> TempDir {
        let base = TempDir::new().unwrap();
        fs::create_dir_all(base.path().join("input")).unwrap();
        fs::create_dir_all(base.path().join("config")).unwrap();
        fs::write(
            base.path().join("config/data-contract.json"),
            r#"{
                "general": {"timeZone": "UTC", "tenantId": "t1", "streamType": "live"},
                "fileDefinitions": {
                    "cond": {
                        "fileType": "csv",
                        "resourceType": "Condition",
                        "groupByKey": "patientInternalId"
                    }
                }
            }"#,
        )
        .unwrap();
        fs::write(
            base.path().join("input/cond.csv"),
            "patientInternalId,conditionCode\np1,E11.9\n",
        )
        .unwrap();
        base
    }

    #[test]
    fn dangling_references_warn_and_fail_strict() {
        let base = conditions_only_base();
        let output = TempDir::new().unwrap();
        let summary = run_convert(&ConvertRequest::directory(base.path(), output.path())).unwrap();
        assert_eq!(summary.resources, 1);

        let strict_output = TempDir::new().unwrap();
        let error =
            run_convert(&ConvertRequest::directory(base.path(), strict_output.path()).strict())
                .unwrap_err()
                .to_string();
        assert!(error.contains("dangling references"), "{error}");
        assert!(error.contains("Patient/p1"), "{error}");
    }

    #[test]
    fn merge_values_unions_arrays_and_keeps_the_first_scalar() {
        let mut base = json!({
            "resourceType": "Patient",
            "id": "p1",
            "gender": "female",
            "name": [{"family": "Smith"}]
        });
        let incoming = json!({
            "resourceType": "Patient",
            "id": "p1",
            "gender": "male",
            "birthDate": "1980-01-02",
            "name": [{"family": "Smith"}, {"family": "Jones"}]
        });
        let conflict = merge_values(&mut base, &incoming);
        assert!(conflict);
        assert_eq!(base["gender"], json!("female"));
        assert_eq!(base["birthDate"], json!("1980-01-02"));
        assert_eq!(
            base["name"],
            json!([{"family": "Smith"}, {"family": "Jones"}])
        );
    }

    #[test]
    fn duplicate_patient_rows_merge_their_fields() {
        let base = base_directory();
        fs::write(
            base.path().join("input/patient.csv"),
            "patientInternalId,nameLast,gender\np1,Smith,\np1,,female\n",
        )
        .unwrap();
        let output = TempDir::new().unwrap();

        let summary = run_convert(&ConvertRequest::directory(base.path(), output.path())).unwrap();
        assert_eq!(summary.deduplicated, 1);

        let content =
            fs::read_to_string(output.path().join("p1/p1-Patient-patient-00001.json")).unwrap();
        let patient: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(patient["name"][0]["family"], json!("Smith"));
        assert_eq!(patient["gender"], json!("female"));
    }

    #[test]
    fn duplicates_merge_inside_bundles_too() {
        let base = base_directory();
        fs::write(
            base.path().join("input/patient.csv"),
            "patientInternalId,nameLast,gender\np1,Smith,\np1,,female\n",
        )
        .unwrap();
        let output = TempDir::new().unwrap();

        let request = ConvertRequest::directory(base.path(), output.path()).bundled("collection");
        let summary = run_convert(&request).unwrap();
        assert_eq!(summary.deduplicated, 1);

        let content = fs::read_to_string(output.path().join("p1/p1-Bundle.json")).unwrap();
        let bundle: serde_json::Value = serde_json::from_str(&content).unwrap();
        let patients: Vec<&serde_json::Value> = bundle["entry"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["resource"]["resourceType"] == json!("Patient"))
            .collect();
        assert_eq!(patients.len(), 1);
        assert_eq!(patients[0]["resource"]["gender"], json!("female"));
        assert_eq!(patients[0]["resource"]["name"][0]["family"], json!("Smith"));
    }

    #[test]
    fn repeated_practitioners_collapse_to_one_resource() {
        let base = TempDir::new().unwrap();
        fs::create_dir_all(base.path().join("input")).unwrap();
        fs::create_dir_all(base.path().join("config")).unwrap();
        fs::write(
            base.path().join("config/data-contract.json"),
            r#"{
                "general": {"timeZone": "UTC", "tenantId": "t1", "streamType": "live"},
                "fileDefinitions": {
                    "obs": {
                        "fileType": "csv",
                        "resourceType": "Observation",
                        "groupByKey": "mrn"
                    }
                }
            }"#,
        )
        .unwrap();
        fs::write(
            base.path().join("input/obs.csv"),
            "mrn,observationCode,practitionerInternalId,practitionerNameLast\n\
             m1,1234-5,doc9,Who\n\
             m1,5678-9,doc9,Who\n",
        )
        .unwrap();
        let output = TempDir::new().unwrap();

        let summary = run_convert(&ConvertRequest::directory(base.path(), output.path())).unwrap();
        assert_eq!(summary.deduplicated, 1);

        let mut practitioners = 0;
        let mut references = std::collections::HashSet::new();
        for entry in fs::read_dir(output.path().join("m1")).unwrap() {
            let content = fs::read_to_string(entry.unwrap().path()).unwrap();
            let resource: serde_json::Value = serde_json::from_str(&content).unwrap();
            if resource["resourceType"] == json!("Practitioner") {
                practitioners += 1;
            }
            if let Some(performer) = resource["performer"][0]["reference"].as_str() {
                references.insert(performer.to_string());
            }
        }
        assert_eq!(practitioners, 1);
        assert_eq!(references.len(), 1);
        assert!(references
            .iter()
            .next()
            .unwrap()
            .starts_with("Practitioner/"));
    }

    #[test]
    fn bundle_mode_writes_one_bundle_per_group_key() {
        let base = base_directory();
        let output = TempDir::new().unwrap();

        let request = ConvertRequest::directory(base.path(), output.path()).bundled("transaction");
        let summary = run_convert(&request).unwrap();
        assert!(summary.resources > 0);

        let content = fs::read_to_string(output.path().join("p1/p1-Bundle.json")).unwrap();
        let bundle: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(bundle["resourceType"], json!("Bundle"));
        assert_eq!(bundle["type"], json!("transaction"));
        assert!(bundle["meta"]["lastUpdated"].is_string());
        let entry = &bundle["entry"][0];
        let resource_type = entry["resource"]["resourceType"].as_str().unwrap();
        let id = entry["resource"]["id"].as_str().unwrap();
        assert_eq!(entry["fullUrl"], json!(format!("{resource_type}/{id}")));
        assert_eq!(entry["request"]["method"], json!("PUT"));
        assert!(output.path().join("p2/p2-Bundle.json").exists());

        let names: Vec<String> = fs::read_dir(output.path().join("p1"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["p1-Bundle.json".to_string()]);
    }

    #[test]
    fn a_collection_bundle_has_no_request_entries() {
        let base = base_directory();
        let output = TempDir::new().unwrap();
        let request = ConvertRequest::directory(base.path(), output.path()).bundled("collection");
        run_convert(&request).unwrap();

        let content = fs::read_to_string(output.path().join("p1/p1-Bundle.json")).unwrap();
        let bundle: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(bundle["type"], json!("collection"));
        assert!(bundle["entry"][0].get("request").is_none());
    }

    #[test]
    fn an_unknown_bundle_type_is_rejected() {
        let base = base_directory();
        let output = TempDir::new().unwrap();
        let request = ConvertRequest::directory(base.path(), output.path()).bundled("searchset");
        let error = run_convert(&request).unwrap_err().to_string();
        assert!(error.contains("searchset"), "{error}");
        assert!(error.contains("collection"), "{error}");
    }

    #[test]
    fn transform_emits_internal_records_as_json_lines() {
        let base = base_directory();
        let request = ConvertRequest::directory(base.path(), Path::new("unused"));
        let mut out = Vec::new();
        let summary = run_transform(&request, &mut out).unwrap();
        assert_eq!(summary.files, 1);
        assert_eq!(summary.rows, 2);

        let lines: Vec<serde_json::Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["nameLast"], json!("Smith"));
        assert_eq!(lines[0]["groupByKey"], json!("p1"));
        assert_eq!(lines[0]["tenantId"], json!("tenant1"));
    }

    #[test]
    fn an_id_key_makes_resource_ids_stable_across_runs() {
        let base = TempDir::new().unwrap();
        fs::create_dir_all(base.path().join("input")).unwrap();
        fs::create_dir_all(base.path().join("config")).unwrap();
        fs::write(
            base.path().join("config/data-contract.json"),
            r#"{
                "general": {"timeZone": "UTC", "tenantId": "t1", "streamType": "live"},
                "fileDefinitions": {
                    "obs": {
                        "fileType": "csv",
                        "resourceType": "Observation",
                        "groupByKey": "mrn",
                        "idKey": "observationCode"
                    }
                }
            }"#,
        )
        .unwrap();
        fs::write(
            base.path().join("input/obs.csv"),
            "mrn,observationCode\nm1,1234-5\n",
        )
        .unwrap();

        let read_ids = |output: &TempDir| -> Vec<String> {
            let mut ids: Vec<String> = fs::read_dir(output.path().join("m1"))
                .unwrap()
                .map(|entry| {
                    let content = fs::read_to_string(entry.unwrap().path()).unwrap();
                    let resource: serde_json::Value = serde_json::from_str(&content).unwrap();
                    resource["id"].as_str().unwrap().to_string()
                })
                .collect();
            ids.sort();
            ids
        };

        let first = TempDir::new().unwrap();
        run_convert(&ConvertRequest::directory(base.path(), first.path())).unwrap();
        let second = TempDir::new().unwrap();
        run_convert(&ConvertRequest::directory(base.path(), second.path())).unwrap();

        let ids = read_ids(&first);
        assert_eq!(ids, read_ids(&second));
        let expected =
            uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, b"Observation:1234-5").to_string();
        assert!(ids.contains(&expected), "{ids:?}");
    }

    #[test]
    fn gzipped_inputs_convert() {
        use std::io::Write as _;
        let base = base_directory();
        fs::remove_file(base.path().join("input/patient.csv")).unwrap();
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder
            .write_all(b"patientInternalId,nameLast,nameFirst\np1,Smith,Ann\n")
            .unwrap();
        fs::write(
            base.path().join("input/patient.csv.gz"),
            encoder.finish().unwrap(),
        )
        .unwrap();
        let output = TempDir::new().unwrap();

        let request = ConvertRequest::directory(base.path(), output.path());
        let summary = run_convert(&request).unwrap();
        assert_eq!(summary.files, 1);
        assert!(summary.resources > 0);
        assert!(output.path().join("p1").is_dir());
    }

    #[test]
    #[serial]
    fn a_scheme_output_writes_through_the_opener() {
        let base = base_directory();
        let memory = Arc::new(crate::opener::MemoryOpener::new(Vec::new()));

        let request = ConvertRequest::directory(base.path(), Path::new("mem://out"))
            .with_opener(memory.clone());
        let summary = run_convert(&request).unwrap();
        assert!(summary.resources > 0);

        let written = memory.written();
        assert_eq!(written.len(), summary.resources);
        assert!(written
            .iter()
            .all(|(name, _)| name.starts_with("mem://out/")));
        let (_, content) = &written[0];
        let resource: serde_json::Value = serde_json::from_slice(content).unwrap();
        assert!(resource.get("resourceType").is_some());
    }

    #[test]
    fn group_keys_cannot_escape_the_output_directory() {
        assert_eq!(safe_segment("../../escaped"), ".._.._escaped");
        assert_eq!(safe_segment("/etc/passwd"), "_etc_passwd");
        assert_eq!(safe_segment(".."), "_");
        assert_eq!(safe_segment("."), "_");
        assert_eq!(safe_segment(""), "_");
        assert_eq!(safe_segment("a\0b"), "a_b");
        assert_eq!(safe_segment("Пациент-1"), "Пациент-1");
        assert_eq!(safe_segment("with space"), "with space");
        assert_eq!(safe_segment("1.2.3"), "1.2.3");
    }

    #[test]
    #[serial]
    fn a_crafted_group_key_stays_inside_the_output_directory() {
        let base = base_directory();
        fs::write(
            base.path().join("input/patient.csv"),
            "patientInternalId,nameLast\n../../escaped,Evil\n",
        )
        .unwrap();
        let root = TempDir::new().unwrap();
        let output = root.path().join("out");

        run_convert(&ConvertRequest::directory(base.path(), &output)).unwrap();

        let written: Vec<PathBuf> = fs::read_dir(&output)
            .unwrap()
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .collect();
        assert_eq!(written.len(), 1);
        assert!(written[0].starts_with(&output));
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    #[serial]
    fn strict_mode_fails_a_run_that_cannot_group() {
        let base = base_directory();
        fs::write(
            base.path().join("config/data-contract.json"),
            CONTRACT.replace(
                r#""groupByKey": "patientInternalId""#,
                r#""groupByKey": "mrn""#,
            ),
        )
        .unwrap();
        let output = TempDir::new().unwrap();

        let lenient = ConvertRequest::directory(base.path(), output.path());
        assert_eq!(run_convert(&lenient).unwrap().resources, 2);

        let strict = ConvertRequest::directory(base.path(), output.path()).strict();
        let error = run_convert(&strict).unwrap_err().to_string();
        assert!(error.contains("mrn"), "{error}");
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
            base.path().join("config/data-contract.json"),
            CONTRACT
                .replace("\"Patient\"", "\"Observation\"")
                .replace("patientInternalId", "mrn"),
        )
        .unwrap();
        fs::write(
            base.path().join("input/patient.csv"),
            "mrn,observationCode\nm1,1234-5\nm1,5678-9\n",
        )
        .unwrap();
        let output = TempDir::new().unwrap();
        let request =
            ConvertRequest::directory(base.path().to_path_buf(), output.path().to_path_buf());

        run_convert(&request).unwrap();
        assert!(output
            .path()
            .join("m1/m1-Observation-patient-00001.json")
            .is_file());
        assert!(output
            .path()
            .join("m1/m1-Observation-patient-00002.json")
            .is_file());
    }

    #[test]
    fn identical_patient_rows_collapse_to_the_first() {
        let base = base_directory();
        fs::write(
            base.path().join("input/patient.csv"),
            "patientInternalId,nameLast\np1,Smith\np1,Jones\n",
        )
        .unwrap();
        let output = TempDir::new().unwrap();

        let summary = run_convert(&ConvertRequest::directory(base.path(), output.path())).unwrap();
        assert_eq!(summary.deduplicated, 1);
        assert!(output
            .path()
            .join("p1/p1-Patient-patient-00001.json")
            .is_file());
        assert!(!output
            .path()
            .join("p1/p1-Patient-patient-00002.json")
            .exists());
    }
}
