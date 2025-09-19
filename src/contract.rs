use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

use crate::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct General {
    pub time_zone: String,
    pub tenant_id: String,
    pub stream_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigning_authority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empty_field_values: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub regex_filenames: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileDefinition {
    #[serde(rename = "fileType")]
    pub file_type: FileType,
    #[serde(default = "default_delimiter", skip_serializing_if = "is_comma")]
    pub value_delimiter: char,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub convert_columns_to_string: bool,
    #[serde(rename = "resourceType")]
    pub resource_type: String,
    #[serde(rename = "groupByKey")]
    pub group_by_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skiprows: Option<SkipRows>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<Headers>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<Task>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

fn default_delimiter() -> char {
    ','
}

fn is_comma(c: &char) -> bool {
    *c == ','
}

fn default_true() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    Csv,
    #[serde(rename = "fixed-width")]
    FixedWidth,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum SkipRows {
    Single(usize),
    Multiple(Vec<usize>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Headers {
    List(Vec<String>),
    Dict(Vec<HeaderDict>),
    WidthMap(WidthMap),
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidthMap(pub Vec<(String, usize)>);

impl WidthMap {
    pub fn entries(&self) -> &[(String, usize)] {
        &self.0
    }
}

impl Serialize for WidthMap {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (name, width) in &self.0 {
            map.serialize_entry(name, width)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for WidthMap {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct WidthMapVisitor;

        impl<'de> serde::de::Visitor<'de> for WidthMapVisitor {
            type Value = WidthMap;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a map of column names to widths")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut access: A,
            ) -> Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some((name, width)) = access.next_entry::<String, usize>()? {
                    entries.push((name, width));
                }
                Ok(WidthMap(entries))
            }
        }

        deserializer.deserialize_map(WidthMapVisitor)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HeaderDict {
    pub name: String,
    pub width: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub task: String,
    #[serde(flatten)]
    pub params: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Contract {
    pub general: General,
    #[serde(rename = "fileDefinitions")]
    pub file_definitions: HashMap<String, FileDefinition>,
}

const VALID_RESOURCE_TYPES: &[&str] = &[
    "Patient",
    "AllergyIntolerance",
    "Condition",
    "Encounter",
    "Immunization",
    "Observation",
    "Location",
    "Organization",
    "Practitioner",
    "Procedure",
    "MedicationUse",
    "MedicationAdministration",
    "MedicationRequest",
    "MedicationStatement",
    "DocumentReference",
    "DiagnosticReport",
    "Unstructured",
    "Basic",
];

#[derive(Deserialize)]
struct RawContract<'a> {
    general: General,
    #[serde(rename = "fileDefinitions", borrow, default)]
    file_definitions: HashMap<String, &'a serde_json::value::RawValue>,
}

fn resolve_definition(raw: &serde_json::value::RawValue) -> Result<FileDefinition, Error> {
    match serde_json::from_str::<String>(raw.get()) {
        Ok(source) => {
            let content = crate::opener::read_to_string(&source)?;
            Ok(serde_json::from_str(without_bom(&content))?)
        }
        Err(_) => Ok(serde_json::from_str(raw.get())?),
    }
}

pub fn without_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

const KNOWN_TASKS: &[&str] = &[
    "add_constant",
    "add_row_num",
    "append_list",
    "build_object_array",
    "change_case",
    "compare_to_date",
    "conditional_column",
    "conditional_column_update",
    "condition_column_with_prerequisite",
    "convert_to_list",
    "copy_columns",
    "filter_to_columns",
    "find_not_null_value",
    "format_date",
    "join_data",
    "map_codes",
    "remove_whitespace_from_columns",
    "rename_columns",
    "replace_text",
    "set_nan_to_none",
    "split_column",
    "split_row",
    "validate_value",
];

impl Contract {
    pub fn load(json: &str) -> Result<Self, Error> {
        let raw: RawContract = serde_json::from_str(without_bom(json))?;
        let mut file_definitions = HashMap::new();
        for (matcher, definition) in raw.file_definitions {
            file_definitions.insert(matcher, resolve_definition(definition)?);
        }

        let contract = Contract {
            general: raw.general,
            file_definitions,
        };
        contract.validate()?;
        Ok(contract)
    }

    fn validate(&self) -> Result<(), Error> {
        self.validate_general()?;
        self.validate_file_definitions()?;
        Ok(())
    }

    fn validate_general(&self) -> Result<(), Error> {
        Tz::from_str(&self.general.time_zone)
            .map_err(|_| Error::InvalidTimezone(self.general.time_zone.clone()))?;

        if self.general.tenant_id.is_empty() {
            return Err(Error::Config("tenantId is required".into()));
        }

        if !matches!(self.general.stream_type.as_str(), "live" | "historical") {
            return Err(Error::InvalidStreamType(self.general.stream_type.clone()));
        }

        Ok(())
    }

    fn validate_file_definitions(&self) -> Result<(), Error> {
        for (matcher, def) in &self.file_definitions {
            self.validate_file_definition(matcher, def)?;
        }
        Ok(())
    }

    fn validate_file_definition(&self, matcher: &str, def: &FileDefinition) -> Result<(), Error> {
        if !VALID_RESOURCE_TYPES.contains(&def.resource_type.as_str()) {
            return Err(Error::UnknownResourceType(def.resource_type.clone()));
        }

        if def.group_by_key.as_deref().unwrap_or("").is_empty() {
            return Err(Error::Config(format!(
                "groupByKey is required for file matcher: {matcher}"
            )));
        }

        if matches!(def.file_type, FileType::FixedWidth)
            && !matches!(
                &def.headers,
                Some(Headers::Dict(_)) | Some(Headers::WidthMap(_))
            )
        {
            return Err(Error::FixedWidthRequiresDictHeaders);
        }

        if let Some(tasks) = &def.tasks {
            for task in tasks {
                if !KNOWN_TASKS.contains(&task.task.as_str()) {
                    return Err(Error::UnknownTask(task.task.clone()));
                }
                self.validate_task_params(&task.task, &task.params)?;
            }
        }

        Ok(())
    }

    fn validate_task_params(
        &self,
        task_name: &str,
        params: &HashMap<String, serde_json::Value>,
    ) -> Result<(), Error> {
        let required_params = match task_name {
            "add_constant" => &["name", "value"][..],
            "add_row_num" => &["starting_index"][..],
            "append_list" => &["source_columns", "target_column"][..],
            "build_object_array" => &["entry_class", "target_column", "entries"][..],
            "change_case" => &["columns", "casing"][..],
            "compare_to_date" => &[
                "column",
                "target_column",
                "compare_date",
                "comparison",
                "true_string",
                "false_string",
            ][..],
            "conditional_column" => &["source_column", "target_column", "condition_map"][..],
            "conditional_column_update" => &["source_column", "target_column", "condition_map"][..],
            "condition_column_with_prerequisite" => &[
                "source_column",
                "target_column",
                "condition_map",
                "prerequisite_column",
                "prerequisite_match",
            ][..],
            "convert_to_list" => &["column", "separator"][..],
            "copy_columns" => &["columns", "target_column"][..],
            "filter_to_columns" => &["source_column", "target_columns", "filters"][..],
            "find_not_null_value" => &["columns", "target_column"][..],
            "format_date" => &["columns", "date_format"][..],
            "join_data" => &[
                "secondary_data_source",
                "join_type",
                "join_on",
                "source_type",
                "reader_params",
            ][..],
            "map_codes" => &["code_map"][..],
            "rename_columns" => &["column_map"][..],
            "replace_text" => &["column_name", "match", "replacement"][..],
            "split_column" => &["column_name", "new_column_names"][..],
            "split_row" => &["columns", "split_column_name", "split_value_column_name"][..],
            "validate_value" => &["column_name", "regex"][..],
            _ => &[][..],
        };

        for param in required_params {
            if !params.contains_key(*param) {
                return Err(Error::MissingTaskParam(format!("{task_name}.{param}")));
            }
        }

        for param in params.keys() {
            let known_params = match task_name {
                "add_constant" => &["name", "value", "discard_if_duplicate"][..],
                "add_row_num" => &["starting_index"][..],
                "append_list" => &["source_columns", "target_column", "discard_if_duplicate"][..],
                "build_object_array" => &["entry_class", "target_column", "entries"][..],
                "change_case" => &["columns", "casing"][..],
                "compare_to_date" => &[
                    "column",
                    "target_column",
                    "compare_date",
                    "comparison",
                    "true_string",
                    "false_string",
                ][..],
                "conditional_column" => &["source_column", "target_column", "condition_map"][..],
                "conditional_column_update" => {
                    &["source_column", "target_column", "condition_map"][..]
                }
                "condition_column_with_prerequisite" => &[
                    "source_column",
                    "target_column",
                    "condition_map",
                    "prerequisite_column",
                    "prerequisite_match",
                ][..],
                "convert_to_list" => &["column", "separator"][..],
                "copy_columns" => &["columns", "target_column", "value_separator"][..],
                "filter_to_columns" => &["source_column", "target_columns", "filters"][..],
                "find_not_null_value" => &["columns", "target_column"][..],
                "format_date" => &["columns", "date_format"][..],
                "join_data" => &[
                    "secondary_data_source",
                    "join_type",
                    "join_on",
                    "source_type",
                    "reader_params",
                ][..],
                "map_codes" => &["code_map", "default"][..],
                "rename_columns" => &["column_map"][..],
                "replace_text" => &["column_name", "match", "replacement", "options"][..],
                "split_column" => &["column_name", "new_column_names", "delimiter", "indices"][..],
                "split_row" => &["columns", "split_column_name", "split_value_column_name"][..],
                "validate_value" => &["column_name", "regex", "no_match_replacement"][..],
                _ => &[][..],
            };
            if !known_params.contains(&param.as_str()) {
                return Err(Error::UnknownTaskParam(format!("{task_name}.{param}")));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod external_definition_tests {
    use crate::contract::Contract;
    use crate::opener::{opener, set_opener, MemoryOpener};
    use serial_test::serial;
    use std::sync::Arc;

    const CONTRACT: &str = r#"{
        "general": {"timeZone": "UTC", "tenantId": "t1", "streamType": "live"},
        "fileDefinitions": {"patient": "patient-definition.json"}
    }"#;

    const DEFINITION: &str = r#"{
        "fileType": "csv",
        "resourceType": "Patient",
        "groupByKey": "patientInternalId"
    }"#;

    #[test]
    #[serial]
    fn external_file_definitions_are_loaded_through_the_opener() {
        let previous = opener();
        set_opener(Arc::new(MemoryOpener::new(vec![(
            "patient-definition.json".into(),
            DEFINITION.as_bytes().to_vec(),
        )])));

        let contract = Contract::load(CONTRACT).unwrap();
        let definition = contract.file_definitions.get("patient").unwrap();
        assert_eq!(definition.resource_type, "Patient");
        assert_eq!(
            definition.group_by_key.as_deref(),
            Some("patientInternalId")
        );

        set_opener(previous);
    }

    #[test]
    #[serial]
    fn a_missing_external_definition_is_reported() {
        let previous = opener();
        set_opener(Arc::new(MemoryOpener::new(Vec::new())));
        assert!(Contract::load(CONTRACT).is_err());
        set_opener(previous);
    }
}
