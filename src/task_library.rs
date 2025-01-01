use crate::contract::{FileType, Headers, SkipRows};
use crate::error::Error;
use crate::opener;
use crate::reader::{read_delimited, read_fixed_width, ReaderParams, RecordBatch};
use crate::tasks::TaskRegistry;
use chrono::{NaiveDate, Utc};
use regex::{Regex, RegexBuilder};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

use std::cell::RefCell;
use std::sync::Arc;

thread_local! {
    /// File and column pairs already reported as leaving values unmapped.
    static REPORTED_UNMAPPED: RefCell<HashSet<(String, String)>> = RefCell::new(HashSet::new());
}

pub const LIST_SEPARATOR: char = '|';

const ENTRY_CLASSES: &[(&str, &[&str])] = &[
    (
        "EncounterStatusHistoryEntry",
        &["status", "start_time", "end_time"],
    ),
    ("IdentifierDetails", &["name", "value", "system"]),
];

pub fn register(registry: &mut TaskRegistry) {
    registry.register("append_list", Arc::new(append_list));
    registry.register("build_object_array", Arc::new(build_object_array));
    registry.register("change_case", Arc::new(change_case));
    registry.register("compare_to_date", Arc::new(compare_to_date));
    registry.register("conditional_column", Arc::new(conditional_column));
    registry.register(
        "conditional_column_update",
        Arc::new(conditional_column_update),
    );
    registry.register(
        "condition_column_with_prerequisite",
        Arc::new(condition_column_with_prerequisite),
    );
    registry.register("convert_to_list", Arc::new(convert_to_list));
    registry.register("filter_to_columns", Arc::new(filter_to_columns));
    registry.register("find_not_null_value", Arc::new(find_not_null_value));
    registry.register("format_date", Arc::new(format_date));
    registry.register("join_data", Arc::new(join_data));
    registry.register("map_codes", Arc::new(map_codes));
    registry.register("rename_columns", Arc::new(rename_columns));
    registry.register("replace_text", Arc::new(replace_text));
    registry.register("split_column", Arc::new(split_column));
    registry.register("split_row", Arc::new(split_row));
    registry.register("validate_value", Arc::new(validate_value));
}

fn missing(task: &str, param: &str) -> Error {
    Error::MissingTaskParam(format!("{task}.{param}"))
}

fn string_param<'a>(
    params: &'a HashMap<String, Value>,
    task: &str,
    name: &str,
) -> Result<&'a str, Error> {
    params
        .get(name)
        .and_then(|value| value.as_str())
        .ok_or_else(|| missing(task, name))
}

fn string_list_param(
    params: &HashMap<String, Value>,
    task: &str,
    name: &str,
) -> Result<Vec<String>, Error> {
    let values = params
        .get(name)
        .and_then(|value| value.as_array())
        .ok_or_else(|| missing(task, name))?;
    Ok(values
        .iter()
        .filter_map(|value| value.as_str().map(|value| value.to_string()))
        .collect())
}

fn map_param<'a>(
    params: &'a HashMap<String, Value>,
    task: &str,
    name: &str,
) -> Result<&'a Map<String, Value>, Error> {
    params
        .get(name)
        .and_then(|value| value.as_object())
        .ok_or_else(|| missing(task, name))
}

fn cell<'a>(batch: &'a RecordBatch, column: &str, row: usize) -> Option<&'a str> {
    batch
        .columns
        .get(column)?
        .get(row)?
        .as_deref()
        .filter(|value| !value.is_empty())
}

fn matched_columns(batch: &RecordBatch, columns: &[String]) -> Vec<String> {
    columns
        .iter()
        .filter(|column| batch.columns.contains_key(*column))
        .cloned()
        .collect()
}

pub fn split_list(value: Option<&str>) -> Vec<String> {
    match value {
        Some(value) if !value.is_empty() => value
            .split(LIST_SEPARATOR)
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty() && item != "None")
            .collect(),
        _ => Vec::new(),
    }
}

pub fn join_list(items: &[String]) -> Option<String> {
    if items.is_empty() {
        None
    } else {
        Some(items.join(&LIST_SEPARATOR.to_string()))
    }
}

fn set_column(batch: &mut RecordBatch, name: &str, values: Vec<Option<String>>) {
    batch.columns.insert(name.to_string(), values);
}

fn read_source(source: &str, params: &ReaderParams) -> Result<RecordBatch, Error> {
    let mut handle = opener::open(source)?;
    match params.file_type {
        FileType::Csv => read_delimited(&mut handle, params),
        FileType::FixedWidth => read_fixed_width(&mut handle, params),
    }
}

const SOURCE_COLUMN: &str = "source_value";
const TARGET_COLUMN: &str = "target_value";

fn read_map_file(source: &str) -> Result<Map<String, Value>, Error> {
    let handle = opener::open(source)?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(handle);

    let headers = reader.headers()?.clone();
    let position = |name: &str| headers.iter().position(|header| header.trim() == name);
    let key_index = position(SOURCE_COLUMN).unwrap_or(0);
    let value_index = position(TARGET_COLUMN).unwrap_or(1);

    let mut mapping = Map::new();
    for record in reader.records() {
        let record = record?;
        let key = match record.get(key_index).map(str::trim) {
            Some(key) if !key.is_empty() => key,
            _ => continue,
        };
        let value = record.get(value_index).map(str::trim).unwrap_or_default();
        let value = if value.is_empty() || value == "null" {
            Value::Null
        } else {
            Value::String(value.to_string())
        };
        mapping.insert(key.to_string(), value);
    }
    Ok(mapping)
}

fn resolve_map(value: &Value, task: &str, name: &str) -> Result<Map<String, Value>, Error> {
    match value {
        Value::Object(map) => Ok(map.clone()),
        Value::String(source) => read_map_file(source),
        _ => Err(missing(task, name)),
    }
}

fn mapped_value(
    mapping: &Map<String, Value>,
    source: Option<&str>,
    fallback: Option<&str>,
) -> Option<String> {
    let mapped = source
        .and_then(|source| mapping.get(source))
        .or_else(|| mapping.get("default"));
    match mapped {
        Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value.clone()),
        Some(value) => Some(value.to_string()),
        None => fallback.map(|value| value.to_string()),
    }
}

fn append_list(batch: &mut RecordBatch, params: &HashMap<String, Value>) -> Result<(), Error> {
    let source_columns = string_list_param(params, "append_list", "source_columns")?;
    let target_column = string_param(params, "append_list", "target_column")?;
    let discard_duplicates = params
        .get("discard_if_duplicate")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    let mut values = Vec::with_capacity(batch.row_count);
    for row in 0..batch.row_count {
        let mut items = split_list(cell(batch, target_column, row));
        for column in &source_columns {
            items.extend(split_list(cell(batch, column, row)));
        }
        if discard_duplicates {
            let mut seen = HashSet::new();
            items.retain(|item| seen.insert(item.clone()));
        }
        items.sort();
        values.push(join_list(&items));
    }
    set_column(batch, target_column, values);
    Ok(())
}

fn build_object_array(
    batch: &mut RecordBatch,
    params: &HashMap<String, Value>,
) -> Result<(), Error> {
    let entry_class = string_param(params, "build_object_array", "entry_class")?;
    let target_column = string_param(params, "build_object_array", "target_column")?;
    let entries = params
        .get("entries")
        .and_then(|value| value.as_array())
        .ok_or_else(|| missing("build_object_array", "entries"))?;

    let fields = ENTRY_CLASSES
        .iter()
        .find(|(name, _)| *name == entry_class)
        .map(|(_, fields)| *fields)
        .ok_or_else(|| Error::Config(format!("unknown entry class: {entry_class}")))?;

    let mut values = Vec::with_capacity(batch.row_count);
    for row in 0..batch.row_count {
        let mut items = split_list(cell(batch, target_column, row));
        for entry in entries {
            let mut parts = Vec::new();
            for field in fields {
                let value = entry.get(*field).and_then(|value| value.as_str());
                let resolved = match value {
                    Some(value) if value.starts_with('$') => {
                        cell(batch, &value[1..], row).unwrap_or("None").to_string()
                    }
                    Some(value) => value.to_string(),
                    None => "None".to_string(),
                };
                parts.push(resolved);
            }
            items.push(parts.join("^"));
        }
        values.push(join_list(&items));
    }
    set_column(batch, target_column, values);
    Ok(())
}

fn change_case(batch: &mut RecordBatch, params: &HashMap<String, Value>) -> Result<(), Error> {
    let columns = string_list_param(params, "change_case", "columns")?;
    let casing = string_param(params, "change_case", "casing")?.to_ascii_uppercase();
    let upper = casing.contains("UPPER");
    let lower = casing.contains("LOWER");
    if upper == lower {
        return Ok(());
    }

    for column in matched_columns(batch, &columns) {
        if let Some(values) = batch.columns.get_mut(&column) {
            for value in values.iter_mut() {
                if let Some(text) = value {
                    *value = Some(if upper {
                        text.to_uppercase()
                    } else {
                        text.to_lowercase()
                    });
                }
            }
        }
    }
    Ok(())
}

fn compare_to_date(batch: &mut RecordBatch, params: &HashMap<String, Value>) -> Result<(), Error> {
    let column = string_param(params, "compare_to_date", "column")?;
    let target_column = string_param(params, "compare_to_date", "target_column")?;
    let compare_date = params
        .get("compare_date")
        .and_then(|value| value.as_str())
        .unwrap_or("TODAY");
    let comparison = params
        .get("comparison")
        .and_then(|value| value.as_str())
        .unwrap_or("DATE_OR_BEFORE")
        .to_ascii_uppercase();
    let true_string = params
        .get("true_string")
        .and_then(|value| value.as_str())
        .unwrap_or("TRUE");
    let false_string = params
        .get("false_string")
        .and_then(|value| value.as_str())
        .unwrap_or("FALSE");

    let target_date = if compare_date.to_ascii_uppercase().contains("TODAY") {
        Utc::now().date_naive()
    } else {
        parse_date(compare_date)
            .ok_or_else(|| Error::Config(format!("invalid compare_date: {compare_date}")))?
    };

    let mut values = Vec::with_capacity(batch.row_count);
    for row in 0..batch.row_count {
        let matched = cell(batch, column, row)
            .and_then(parse_date)
            .map(|value| compare(&comparison, value, target_date))
            .unwrap_or(false);
        values.push(Some(
            if matched { true_string } else { false_string }.to_string(),
        ));
    }
    set_column(batch, target_column, values);
    Ok(())
}

fn compare(comparison: &str, left: NaiveDate, right: NaiveDate) -> bool {
    if comparison.contains("DATE_OR_BEFORE") || comparison.contains("LE") {
        left <= right
    } else if comparison.contains("BEFORE_DATE") || comparison.contains("LT") {
        left < right
    } else if comparison.contains("NOT_EQUAL") || comparison.contains("NE") {
        left != right
    } else if comparison.contains("EQUAL") || comparison.contains("EQ") {
        left == right
    } else if comparison.contains("DATE_OR_AFTER") || comparison.contains("GE") {
        left >= right
    } else if comparison.contains("AFTER_DATE") || comparison.contains("GT") {
        left > right
    } else {
        false
    }
}

fn parse_date(value: &str) -> Option<NaiveDate> {
    crate::fhirutils::builders::date(value)
        .and_then(|date| NaiveDate::parse_from_str(&date, "%Y-%m-%d").ok())
}

fn conditional_column(
    batch: &mut RecordBatch,
    params: &HashMap<String, Value>,
) -> Result<(), Error> {
    let source_column = string_param(params, "conditional_column", "source_column")?;
    let target_column = string_param(params, "conditional_column", "target_column")?;
    let mapping = resolve_map(
        params
            .get("condition_map")
            .ok_or_else(|| missing("conditional_column", "condition_map"))?,
        "conditional_column",
        "condition_map",
    )?;

    if !batch.columns.contains_key(source_column) {
        return Ok(());
    }

    let mut values = Vec::with_capacity(batch.row_count);
    for row in 0..batch.row_count {
        let source = cell(batch, source_column, row);
        values.push(mapped_value(&mapping, source, source));
    }
    set_column(batch, target_column, values);
    Ok(())
}

fn conditional_column_update(
    batch: &mut RecordBatch,
    params: &HashMap<String, Value>,
) -> Result<(), Error> {
    let source_column = string_param(params, "conditional_column_update", "source_column")?;
    let target_column = string_param(params, "conditional_column_update", "target_column")?;
    let mapping = resolve_map(
        params
            .get("condition_map")
            .ok_or_else(|| missing("conditional_column_update", "condition_map"))?,
        "conditional_column_update",
        "condition_map",
    )?;

    if !batch.columns.contains_key(source_column) {
        return Ok(());
    }

    let mut values = Vec::with_capacity(batch.row_count);
    for row in 0..batch.row_count {
        let source = cell(batch, source_column, row);
        let target = cell(batch, target_column, row);
        values.push(mapped_value(&mapping, source, target));
    }
    set_column(batch, target_column, values);
    Ok(())
}

fn condition_column_with_prerequisite(
    batch: &mut RecordBatch,
    params: &HashMap<String, Value>,
) -> Result<(), Error> {
    let task = "condition_column_with_prerequisite";
    let source_column = string_param(params, task, "source_column")?;
    let target_column = string_param(params, task, "target_column")?;
    let prerequisite_column = string_param(params, task, "prerequisite_column")?;
    let prerequisite_match = params
        .get("prerequisite_match")
        .and_then(|value| value.as_str());
    let mapping = resolve_map(
        params
            .get("condition_map")
            .ok_or_else(|| missing(task, "condition_map"))?,
        task,
        "condition_map",
    )?;

    if !batch.columns.contains_key(source_column) {
        return Ok(());
    }
    let pattern = match prerequisite_match {
        Some(pattern) => {
            Some(Regex::new(pattern).map_err(|error| Error::Config(error.to_string()))?)
        }
        None => None,
    };

    let mut values = Vec::with_capacity(batch.row_count);
    for row in 0..batch.row_count {
        let source = cell(batch, source_column, row);
        let target = cell(batch, target_column, row);
        let prerequisite = cell(batch, prerequisite_column, row);
        let matched = match (&pattern, prerequisite) {
            (None, prerequisite) => prerequisite.is_none(),
            (Some(_), None) => false,
            (Some(pattern), Some(value)) => pattern.is_match(value),
        };
        if matched && source.is_some() {
            values.push(mapped_value(&mapping, source, target));
        } else {
            values.push(target.map(|value| value.to_string()));
        }
    }
    set_column(batch, target_column, values);
    Ok(())
}

fn convert_to_list(batch: &mut RecordBatch, params: &HashMap<String, Value>) -> Result<(), Error> {
    let column = string_param(params, "convert_to_list", "column")?;
    let separator = string_param(params, "convert_to_list", "separator")?;
    if !batch.columns.contains_key(column) {
        return Ok(());
    }

    let mut values = Vec::with_capacity(batch.row_count);
    for row in 0..batch.row_count {
        let value = cell(batch, column, row);
        let items: Vec<String> = match value {
            Some(value) if separator.is_empty() => vec![value.to_string()],
            Some(value) => value
                .split(separator)
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect(),
            None => Vec::new(),
        };
        values.push(join_list(&items));
    }
    set_column(batch, column, values);
    Ok(())
}

fn filter_to_columns(
    batch: &mut RecordBatch,
    params: &HashMap<String, Value>,
) -> Result<(), Error> {
    let source_column = string_param(params, "filter_to_columns", "source_column")?;
    let target_columns = string_list_param(params, "filter_to_columns", "target_columns")?;
    let filters = params
        .get("filters")
        .and_then(|value| value.as_array())
        .ok_or_else(|| missing("filter_to_columns", "filters"))?;

    if !matched_columns(batch, &target_columns).is_empty() {
        return Ok(());
    }
    if target_columns.len() != filters.len() && target_columns.len() != filters.len() + 1 {
        return Err(Error::Config(
            "filter_to_columns needs one filter per target column, or one fewer".into(),
        ));
    }
    if !batch.columns.contains_key(source_column) {
        return Ok(());
    }

    let filter_sets: Vec<HashSet<String>> = filters
        .iter()
        .map(|filter| {
            filter
                .as_array()
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|value| value.as_str().map(|value| value.to_string()))
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();

    for (index, filter) in filter_sets.iter().enumerate() {
        let mut values = Vec::with_capacity(batch.row_count);
        for row in 0..batch.row_count {
            let value = cell(batch, source_column, row);
            values.push(match value {
                Some(value) if filter.contains(value) => Some(value.to_string()),
                _ => None,
            });
        }
        set_column(batch, &target_columns[index], values);
    }

    if target_columns.len() > filter_sets.len() {
        let combined: HashSet<&String> = filter_sets.iter().flatten().collect();
        let mut values = Vec::with_capacity(batch.row_count);
        for row in 0..batch.row_count {
            let value = cell(batch, source_column, row);
            values.push(match value {
                Some(value) if !combined.contains(&value.to_string()) => Some(value.to_string()),
                _ => None,
            });
        }
        set_column(batch, &target_columns[target_columns.len() - 1], values);
    }
    Ok(())
}

fn find_not_null_value(
    batch: &mut RecordBatch,
    params: &HashMap<String, Value>,
) -> Result<(), Error> {
    let columns = string_list_param(params, "find_not_null_value", "columns")?;
    let target_column = string_param(params, "find_not_null_value", "target_column")?;
    let matched = matched_columns(batch, &columns);
    if matched.is_empty() {
        return Ok(());
    }

    let mut values = Vec::with_capacity(batch.row_count);
    for row in 0..batch.row_count {
        values.push(
            matched
                .iter()
                .find_map(|column| cell(batch, column, row))
                .map(|value| value.to_string()),
        );
    }
    set_column(batch, target_column, values);
    Ok(())
}

fn format_date(batch: &mut RecordBatch, params: &HashMap<String, Value>) -> Result<(), Error> {
    let columns = string_list_param(params, "format_date", "columns")?;
    let date_format = params
        .get("date_format")
        .and_then(|value| value.as_str())
        .unwrap_or("%Y-%m-%d");

    for column in matched_columns(batch, &columns) {
        let mut values = Vec::with_capacity(batch.row_count);
        for row in 0..batch.row_count {
            let value = cell(batch, &column, row);
            values.push(match value.and_then(parse_date) {
                Some(date) => Some(date.format(date_format).to_string()),
                None => value.map(|value| value.to_string()),
            });
        }
        set_column(batch, &column, values);
    }
    Ok(())
}

fn map_codes(batch: &mut RecordBatch, params: &HashMap<String, Value>) -> Result<(), Error> {
    let code_map = map_param(params, "map_codes", "code_map")?;
    for (column, mapping) in code_map {
        if !batch.columns.contains_key(column) {
            continue;
        }
        let mapping = resolve_map(mapping, "map_codes", "code_map")?;
        let mut values = Vec::with_capacity(batch.row_count);
        let mut unmapped: Vec<String> = Vec::new();
        for row in 0..batch.row_count {
            let value = cell(batch, column, row);
            if let Some(value) = value {
                if !mapping.contains_key(value) && !mapping.contains_key("default") {
                    unmapped.push(value.to_string());
                }
            }
            values.push(mapped_value(&mapping, value, value));
        }
        report_unmapped(batch, column, unmapped);
        set_column(batch, column, values);
    }
    Ok(())
}

/// Warns once per file and column about values the code map passes through unchanged.
fn report_unmapped(batch: &RecordBatch, column: &str, mut unmapped: Vec<String>) {
    if unmapped.is_empty() {
        return;
    }
    unmapped.sort();
    unmapped.dedup();
    let file = cell(batch, "filePath", 0).unwrap_or_default().to_string();
    let first_time =
        REPORTED_UNMAPPED.with(|seen| seen.borrow_mut().insert((file.clone(), column.to_string())));
    if !first_time {
        return;
    }
    let shown: Vec<&str> = unmapped.iter().take(5).map(String::as_str).collect();
    let rest = unmapped.len().saturating_sub(shown.len());
    let more = if rest > 0 {
        format!(" and {rest} more")
    } else {
        String::new()
    };
    tracing::warn!(
        file = %file,
        column,
        "code map leaves values unchanged: {}{more}",
        shown.join(", ")
    );
}

fn rename_columns(batch: &mut RecordBatch, params: &HashMap<String, Value>) -> Result<(), Error> {
    let column_map = map_param(params, "rename_columns", "column_map")?;
    for (from, to) in column_map {
        let to = match to.as_str() {
            Some(to) => to,
            None => continue,
        };
        if let Some(values) = batch.columns.remove(from) {
            batch.columns.insert(to.to_string(), values);
        }
    }
    Ok(())
}

fn replace_text(batch: &mut RecordBatch, params: &HashMap<String, Value>) -> Result<(), Error> {
    let column_name = string_param(params, "replace_text", "column_name")?;
    let match_value = string_param(params, "replace_text", "match")?;
    let replacement = string_param(params, "replace_text", "replacement")?;
    let options = params
        .get("options")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_ascii_uppercase();

    if !batch.columns.contains_key(column_name) {
        return Ok(());
    }

    let case_insensitive = options.contains("CASE_INSENSITIVE");
    let patterns: Vec<Regex> = if options.contains("REGEX") {
        vec![build_regex(match_value, case_insensitive)?]
    } else {
        let escaped = regex::escape(match_value);
        let begin = options.contains("BEGIN");
        let end = options.contains("END");
        if begin && end {
            vec![
                build_regex(&format!("{escaped}$"), case_insensitive)?,
                build_regex(&format!("^{escaped}"), case_insensitive)?,
            ]
        } else {
            let prefix = if begin { "^" } else { "" };
            let suffix = if end { "$" } else { "" };
            vec![build_regex(
                &format!("{prefix}{escaped}{suffix}"),
                case_insensitive,
            )?]
        }
    };

    let mut values = Vec::with_capacity(batch.row_count);
    for row in 0..batch.row_count {
        let value = cell(batch, column_name, row);
        values.push(value.map(|value| {
            let mut replaced = value.to_string();
            for pattern in &patterns {
                replaced = pattern.replace_all(&replaced, replacement).to_string();
            }
            replaced
        }));
    }
    set_column(batch, column_name, values);
    Ok(())
}

fn build_regex(pattern: &str, case_insensitive: bool) -> Result<Regex, Error> {
    RegexBuilder::new(pattern)
        .case_insensitive(case_insensitive)
        .build()
        .map_err(|error| Error::Config(error.to_string()))
}

fn split_column(batch: &mut RecordBatch, params: &HashMap<String, Value>) -> Result<(), Error> {
    let column_name = string_param(params, "split_column", "column_name")?;
    let new_column_names = string_list_param(params, "split_column", "new_column_names")?;
    let delimiter = params.get("delimiter").and_then(|value| value.as_str());
    let indices = params.get("indices").and_then(|value| value.as_array());

    if !matched_columns(batch, &new_column_names).is_empty() {
        return Ok(());
    }
    if !batch.columns.contains_key(column_name) {
        return Ok(());
    }

    match (delimiter, indices) {
        (Some(delimiter), _) => {
            for (index, name) in new_column_names.iter().enumerate() {
                let mut values = Vec::with_capacity(batch.row_count);
                for row in 0..batch.row_count {
                    values.push(
                        cell(batch, column_name, row)
                            .and_then(|value| value.split(delimiter).nth(index))
                            .map(|value| value.to_string()),
                    );
                }
                set_column(batch, name, values);
            }
            Ok(())
        }
        (None, Some(indices)) => {
            if indices.len() != new_column_names.len() {
                return Err(Error::Config(
                    "split_column needs one index pair per new column".into(),
                ));
            }
            for (position, bounds) in indices.iter().enumerate() {
                let bounds = bounds
                    .as_array()
                    .ok_or_else(|| Error::Config("split_column indices must be arrays".into()))?;
                if bounds.is_empty() || bounds.len() > 2 {
                    return Err(Error::Config(
                        "split_column index boundaries hold one or two entries".into(),
                    ));
                }
                let start = bounds[0].as_u64().unwrap_or_default() as usize;
                let end = bounds
                    .get(1)
                    .and_then(|value| value.as_u64())
                    .map(|value| value as usize);

                let mut values = Vec::with_capacity(batch.row_count);
                for row in 0..batch.row_count {
                    values.push(cell(batch, column_name, row).map(|value| {
                        let chars: Vec<char> = value.chars().collect();
                        let start = start.min(chars.len());
                        let end = end.unwrap_or(chars.len()).min(chars.len()).max(start);
                        chars[start..end].iter().collect::<String>()
                    }));
                }
                set_column(batch, &new_column_names[position], values);
            }
            Ok(())
        }
        (None, None) => Err(missing("split_column", "delimiter")),
    }
}

fn split_row(batch: &mut RecordBatch, params: &HashMap<String, Value>) -> Result<(), Error> {
    let columns = string_list_param(params, "split_row", "columns")?;
    let split_column_name = string_param(params, "split_row", "split_column_name")?;
    let split_value_column_name = string_param(params, "split_row", "split_value_column_name")?;
    let matched = matched_columns(batch, &columns);
    if matched.is_empty() {
        return Ok(());
    }

    let kept: Vec<String> = batch
        .column_names()
        .into_iter()
        .filter(|name| !matched.contains(name))
        .collect();

    let mut result = RecordBatch::new();
    for row in 0..batch.row_count {
        for column in &matched {
            let mut new_row = HashMap::new();
            for name in &kept {
                new_row.insert(
                    name.clone(),
                    cell(batch, name, row).map(|value| value.to_string()),
                );
            }
            new_row.insert(split_column_name.to_string(), Some(column.clone()));
            new_row.insert(
                split_value_column_name.to_string(),
                cell(batch, column, row).map(|value| value.to_string()),
            );
            result.add_row(new_row);
        }
    }
    *batch = result;
    Ok(())
}

fn validate_value(batch: &mut RecordBatch, params: &HashMap<String, Value>) -> Result<(), Error> {
    let column_name = string_param(params, "validate_value", "column_name")?;
    let regex = string_param(params, "validate_value", "regex")?;
    let replacement = params
        .get("no_match_replacement")
        .and_then(|value| value.as_str());
    if !batch.columns.contains_key(column_name) {
        return Ok(());
    }

    let pattern = build_regex(regex, false)?;
    let mut values = Vec::with_capacity(batch.row_count);
    for row in 0..batch.row_count {
        let value = cell(batch, column_name, row);
        values.push(match value {
            Some(value) if pattern.is_match(value) => Some(value.to_string()),
            _ => replacement.map(|replacement| replacement.to_string()),
        });
    }
    set_column(batch, column_name, values);
    Ok(())
}

fn join_data(batch: &mut RecordBatch, params: &HashMap<String, Value>) -> Result<(), Error> {
    let source = string_param(params, "join_data", "secondary_data_source")?;
    let join_type = string_param(params, "join_data", "join_type")?.to_ascii_lowercase();
    let join_on = string_param(params, "join_data", "join_on")?;
    let source_type = params
        .get("source_type")
        .and_then(|value| value.as_str())
        .unwrap_or("csv");
    let reader_params = params
        .get("reader_params")
        .and_then(|value| value.as_object());

    let secondary = read_secondary(source, source_type, reader_params)?;
    if !secondary.columns.contains_key(join_on) || !batch.columns.contains_key(join_on) {
        return Err(Error::Config(format!(
            "join_data column not found in both sources: {join_on}"
        )));
    }

    let secondary_columns: Vec<String> = secondary
        .column_names()
        .into_iter()
        .filter(|name| name != join_on)
        .collect();
    let primary_columns = batch.column_names();

    let mut result = RecordBatch::new();
    let mut matched_secondary: HashSet<usize> = HashSet::new();

    for row in 0..batch.row_count {
        let key = cell(batch, join_on, row);
        let matches: Vec<usize> = (0..secondary.row_count)
            .filter(|index| key.is_some() && cell(&secondary, join_on, *index) == key)
            .collect();

        if matches.is_empty() {
            if join_type == "inner" || join_type == "right" {
                continue;
            }
            let mut new_row = primary_row(batch, &primary_columns, row);
            for column in &secondary_columns {
                new_row.insert(column.clone(), None);
            }
            result.add_row(new_row);
            continue;
        }

        for index in matches {
            matched_secondary.insert(index);
            let mut new_row = primary_row(batch, &primary_columns, row);
            for column in &secondary_columns {
                new_row.insert(
                    column.clone(),
                    cell(&secondary, column, index).map(|value| value.to_string()),
                );
            }
            result.add_row(new_row);
        }
    }

    if join_type == "right" || join_type == "outer" {
        for index in 0..secondary.row_count {
            if matched_secondary.contains(&index) {
                continue;
            }
            let mut new_row = HashMap::new();
            for column in &primary_columns {
                new_row.insert(column.clone(), None);
            }
            for column in &secondary_columns {
                new_row.insert(
                    column.clone(),
                    cell(&secondary, column, index).map(|value| value.to_string()),
                );
            }
            new_row.insert(
                join_on.to_string(),
                cell(&secondary, join_on, index).map(|value| value.to_string()),
            );
            result.add_row(new_row);
        }
    }

    *batch = result;
    Ok(())
}

fn primary_row(
    batch: &RecordBatch,
    columns: &[String],
    row: usize,
) -> HashMap<String, Option<String>> {
    let mut values = HashMap::new();
    for column in columns {
        values.insert(
            column.clone(),
            cell(batch, column, row).map(|value| value.to_string()),
        );
    }
    values
}

fn read_secondary(
    source: &str,
    source_type: &str,
    reader_params: Option<&Map<String, Value>>,
) -> Result<RecordBatch, Error> {
    let file_type = if source_type == "fixed-width" {
        FileType::FixedWidth
    } else {
        FileType::Csv
    };

    let mut params = ReaderParams {
        file_type,
        value_delimiter: ',',
        skiprows: None,
        headers: None,
        empty_field_values: None,
    };

    if let Some(reader_params) = reader_params {
        if let Some(delimiter) = reader_params
            .get("valueDelimiter")
            .or_else(|| reader_params.get("sep"))
            .and_then(|value| value.as_str())
            .and_then(|value| value.chars().next())
        {
            params.value_delimiter = delimiter;
        }
        if let Some(skiprows) = reader_params.get("skiprows") {
            params.skiprows = serde_json::from_value::<SkipRows>(skiprows.clone()).ok();
        }
        if let Some(headers) = reader_params.get("headers") {
            params.headers = serde_json::from_value::<Headers>(headers.clone()).ok();
        }
    }

    read_source(source, &params)
}

#[cfg(test)]
mod tests {
    use crate::reader::RecordBatch;
    use crate::task_library::REPORTED_UNMAPPED;
    use crate::tasks::{execute_task_chain, TaskRegistry};
    use serde_json::{json, Value};
    use std::collections::HashMap;

    fn batch(columns: &[(&str, &[Option<&str>])]) -> RecordBatch {
        let mut batch = RecordBatch::new();
        let rows = columns.first().map(|(_, values)| values.len()).unwrap_or(0);
        for (name, values) in columns {
            batch.columns.insert(
                name.to_string(),
                values
                    .iter()
                    .map(|value| value.map(|value| value.to_string()))
                    .collect(),
            );
        }
        batch.row_count = rows;
        batch
    }

    fn run(batch: &mut RecordBatch, task: &str, params: Value) {
        let registry = TaskRegistry::new();
        let params: HashMap<String, Value> = params
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        let errors = execute_task_chain(
            batch,
            &[crate::contract::Task {
                task: task.to_string(),
                params,
            }],
            &registry,
        );
        assert!(errors.is_empty(), "task {task} failed: {errors:?}");
    }

    fn column(batch: &RecordBatch, name: &str) -> Vec<Option<String>> {
        batch.columns.get(name).cloned().unwrap_or_default()
    }

    #[test]
    fn append_list_merges_sorts_and_deduplicates() {
        let mut data = batch(&[
            ("a", &[Some("x|y"), Some("z")]),
            ("b", &[Some("y"), None]),
            ("target", &[Some("w"), None]),
        ]);
        run(
            &mut data,
            "append_list",
            json!({"source_columns": ["a", "b"], "target_column": "target", "discard_if_duplicate": true}),
        );
        assert_eq!(
            column(&data, "target"),
            vec![Some("w|x|y".into()), Some("z".into())]
        );
    }

    #[test]
    fn build_object_array_uses_the_entry_class_field_order() {
        let mut data = batch(&[
            ("status", &[Some("planned")]),
            ("start", &[Some("2021-06-01")]),
        ]);
        run(
            &mut data,
            "build_object_array",
            json!({
                "entry_class": "EncounterStatusHistoryEntry",
                "target_column": "encounterStatusHistory",
                "entries": [{"status": "$status", "start_time": "$start", "end_time": "None"}]
            }),
        );
        assert_eq!(
            column(&data, "encounterStatusHistory"),
            vec![Some("planned^2021-06-01^None".into())]
        );
    }

    #[test]
    fn change_case_applies_a_single_casing() {
        let mut data = batch(&[("a", &[Some("Ab"), None])]);
        run(
            &mut data,
            "change_case",
            json!({"columns": ["a"], "casing": "UPPER"}),
        );
        assert_eq!(column(&data, "a"), vec![Some("AB".into()), None]);

        run(
            &mut data,
            "change_case",
            json!({"columns": ["a"], "casing": "LOWER"}),
        );
        assert_eq!(column(&data, "a"), vec![Some("ab".into()), None]);

        run(
            &mut data,
            "change_case",
            json!({"columns": ["a"], "casing": "UPPER LOWER"}),
        );
        assert_eq!(column(&data, "a"), vec![Some("ab".into()), None]);
    }

    #[test]
    fn compare_to_date_maps_to_result_strings() {
        let mut data = batch(&[("d", &[Some("2020-01-01"), Some("2030-01-01"), None])]);
        run(
            &mut data,
            "compare_to_date",
            json!({
                "column": "d",
                "target_column": "expired",
                "compare_date": "2025-01-01",
                "comparison": "LT",
                "true_string": "yes",
                "false_string": "no"
            }),
        );
        assert_eq!(
            column(&data, "expired"),
            vec![Some("yes".into()), Some("no".into()), Some("no".into())]
        );
    }

    #[test]
    fn conditional_columns_map_values_with_defaults() {
        let mut data = batch(&[("sex", &[Some("F"), Some("X"), None])]);
        run(
            &mut data,
            "conditional_column",
            json!({
                "source_column": "sex",
                "target_column": "gender",
                "condition_map": {"F": "female", "default": "unknown"}
            }),
        );
        assert_eq!(
            column(&data, "gender"),
            vec![
                Some("female".into()),
                Some("unknown".into()),
                Some("unknown".into())
            ]
        );
    }

    #[test]
    fn conditional_column_update_keeps_the_target_when_unmapped() {
        let mut data = batch(&[
            ("sex", &[Some("F"), Some("X")]),
            ("gender", &[Some("old"), Some("keep")]),
        ]);
        run(
            &mut data,
            "conditional_column_update",
            json!({
                "source_column": "sex",
                "target_column": "gender",
                "condition_map": {"F": "female"}
            }),
        );
        assert_eq!(
            column(&data, "gender"),
            vec![Some("female".into()), Some("keep".into())]
        );
    }

    #[test]
    fn prerequisite_gates_the_conditional_update() {
        let mut data = batch(&[
            ("code", &[Some("A"), Some("A")]),
            ("target", &[Some("t1"), Some("t2")]),
            ("kind", &[Some("lab"), Some("other")]),
        ]);
        run(
            &mut data,
            "condition_column_with_prerequisite",
            json!({
                "source_column": "code",
                "target_column": "target",
                "condition_map": {"A": "mapped"},
                "prerequisite_column": "kind",
                "prerequisite_match": "^lab$"
            }),
        );
        assert_eq!(
            column(&data, "target"),
            vec![Some("mapped".into()), Some("t2".into())]
        );
    }

    #[test]
    fn convert_to_list_splits_on_the_separator() {
        let mut data = batch(&[("codes", &[Some("a,b , c"), None])]);
        run(
            &mut data,
            "convert_to_list",
            json!({"column": "codes", "separator": ","}),
        );
        assert_eq!(column(&data, "codes"), vec![Some("a|b|c".into()), None]);

        let mut single = batch(&[("codes", &[Some("a,b")])]);
        run(
            &mut single,
            "convert_to_list",
            json!({"column": "codes", "separator": ""}),
        );
        assert_eq!(column(&single, "codes"), vec![Some("a,b".into())]);
    }

    #[test]
    fn filter_to_columns_partitions_values() {
        let mut data = batch(&[("code", &[Some("a"), Some("b"), Some("c")])]);
        run(
            &mut data,
            "filter_to_columns",
            json!({
                "source_column": "code",
                "target_columns": ["first", "rest"],
                "filters": [["a"]]
            }),
        );
        assert_eq!(column(&data, "first"), vec![Some("a".into()), None, None]);
        assert_eq!(
            column(&data, "rest"),
            vec![None, Some("b".into()), Some("c".into())]
        );
    }

    #[test]
    fn find_not_null_value_takes_the_first_present_value() {
        let mut data = batch(&[("a", &[None, Some("a2")]), ("b", &[Some("b1"), Some("b2")])]);
        run(
            &mut data,
            "find_not_null_value",
            json!({"columns": ["a", "b"], "target_column": "found"}),
        );
        assert_eq!(
            column(&data, "found"),
            vec![Some("b1".into()), Some("a2".into())]
        );
    }

    #[test]
    fn format_date_rewrites_matched_columns() {
        let mut data = batch(&[("birthDate", &[Some("01/02/1980"), Some("bad"), None])]);
        run(
            &mut data,
            "format_date",
            json!({"columns": ["birthDate"], "date_format": "%Y-%m-%d"}),
        );
        assert_eq!(
            column(&data, "birthDate"),
            vec![Some("1980-01-02".into()), Some("bad".into()), None]
        );
    }

    #[test]
    fn map_codes_reports_values_it_leaves_unchanged_once_per_file_and_column() {
        REPORTED_UNMAPPED.with(|seen| seen.borrow_mut().clear());
        let mut data = batch(&[
            ("sex", &[Some("M"), Some("Z")]),
            ("filePath", &[Some("a.csv"), Some("a.csv")]),
        ]);
        run(
            &mut data,
            "map_codes",
            json!({"code_map": {"sex": {"M": "male"}}}),
        );

        assert_eq!(
            column(&data, "sex"),
            vec![Some("male".into()), Some("Z".into())]
        );
        let reported = REPORTED_UNMAPPED.with(|seen| seen.borrow().len());
        assert_eq!(reported, 1);

        run(
            &mut data,
            "map_codes",
            json!({"code_map": {"sex": {"M": "male"}}}),
        );
        let repeated = REPORTED_UNMAPPED.with(|seen| seen.borrow().len());
        assert_eq!(repeated, 1, "a second chunk must not add a second report");
    }

    #[test]
    fn map_codes_stays_quiet_when_the_map_has_a_default() {
        REPORTED_UNMAPPED.with(|seen| seen.borrow_mut().clear());
        let mut data = batch(&[("sex", &[Some("Z")]), ("filePath", &[Some("b.csv")])]);
        run(
            &mut data,
            "map_codes",
            json!({"code_map": {"sex": {"M": "male", "default": "unknown"}}}),
        );

        assert_eq!(column(&data, "sex"), vec![Some("unknown".into())]);
        assert!(REPORTED_UNMAPPED.with(|seen| seen.borrow().is_empty()));
    }

    #[test]
    fn map_codes_maps_column_values() {
        let mut data = batch(&[("sex", &[Some("M"), Some("Z")])]);
        run(
            &mut data,
            "map_codes",
            json!({"code_map": {"sex": {"M": "male", "default": "unknown"}}}),
        );
        assert_eq!(
            column(&data, "sex"),
            vec![Some("male".into()), Some("unknown".into())]
        );
    }

    #[test]
    fn rename_columns_moves_values() {
        let mut data = batch(&[("sex", &[Some("M")])]);
        run(
            &mut data,
            "rename_columns",
            json!({"column_map": {"sex": "gender"}}),
        );
        assert_eq!(column(&data, "gender"), vec![Some("M".into())]);
        assert!(!data.columns.contains_key("sex"));
    }

    #[test]
    fn replace_text_honours_the_options() {
        let mut data = batch(&[("a", &[Some("abc abc"), None])]);
        run(
            &mut data,
            "replace_text",
            json!({"column_name": "a", "match": "abc", "replacement": "x"}),
        );
        assert_eq!(column(&data, "a"), vec![Some("x x".into()), None]);

        let mut anchored = batch(&[("a", &[Some("abc abc")])]);
        run(
            &mut anchored,
            "replace_text",
            json!({"column_name": "a", "match": "ABC", "replacement": "x", "options": "BEGIN CASE_INSENSITIVE"}),
        );
        assert_eq!(column(&anchored, "a"), vec![Some("x abc".into())]);

        let mut pattern = batch(&[("a", &[Some("a1b2")])]);
        run(
            &mut pattern,
            "replace_text",
            json!({"column_name": "a", "match": "[0-9]", "replacement": "", "options": "REGEX"}),
        );
        assert_eq!(column(&pattern, "a"), vec![Some("ab".into())]);
    }

    #[test]
    fn split_column_supports_delimiters_and_indices() {
        let mut delimited = batch(&[("name", &[Some("Ann,Smith"), Some("Solo")])]);
        run(
            &mut delimited,
            "split_column",
            json!({"column_name": "name", "new_column_names": ["first", "last"], "delimiter": ","}),
        );
        assert_eq!(
            column(&delimited, "first"),
            vec![Some("Ann".into()), Some("Solo".into())]
        );
        assert_eq!(column(&delimited, "last"), vec![Some("Smith".into()), None]);

        let mut fixed = batch(&[("code", &[Some("ABCDE")])]);
        run(
            &mut fixed,
            "split_column",
            json!({"column_name": "code", "new_column_names": ["head", "tail"], "indices": [[0, 2], [2]]}),
        );
        assert_eq!(column(&fixed, "head"), vec![Some("AB".into())]);
        assert_eq!(column(&fixed, "tail"), vec![Some("CDE".into())]);
    }

    #[test]
    fn split_row_melts_columns_into_rows() {
        let mut data = batch(&[
            ("patient", &[Some("p1")]),
            ("height", &[Some("180")]),
            ("weight", &[Some("80")]),
        ]);
        run(
            &mut data,
            "split_row",
            json!({
                "columns": ["height", "weight"],
                "split_column_name": "observationCodeText",
                "split_value_column_name": "observationValue"
            }),
        );
        assert_eq!(data.row_count, 2);
        assert_eq!(
            column(&data, "patient"),
            vec![Some("p1".into()), Some("p1".into())]
        );
        let mut labels = column(&data, "observationCodeText");
        labels.sort();
        assert_eq!(labels, vec![Some("height".into()), Some("weight".into())]);
    }

    #[test]
    #[serial_test::serial]
    fn map_codes_reads_a_mapping_file() {
        let previous = crate::opener::opener();
        crate::opener::set_opener(std::sync::Arc::new(crate::opener::MemoryOpener::new(vec![
            ("sex.csv".into(), b"key,value\nM,male\nF,female\n".to_vec()),
        ])));

        let mut data = batch(&[("sex", &[Some("M"), Some("F"), Some("Z")])]);
        run(
            &mut data,
            "map_codes",
            json!({"code_map": {"sex": "sex.csv"}}),
        );
        assert_eq!(
            column(&data, "sex"),
            vec![Some("male".into()), Some("female".into()), Some("Z".into())]
        );

        crate::opener::set_opener(previous);
    }

    #[test]
    #[serial_test::serial]
    fn mapping_files_use_named_or_positional_columns() {
        let previous = crate::opener::opener();
        crate::opener::set_opener(std::sync::Arc::new(crate::opener::MemoryOpener::new(vec![
            (
                "named.csv".into(),
                b"target_value,source_value\nmale,M\n,F\n".to_vec(),
            ),
            (
                "positional.csv".into(),
                b"zebra,alpha\nM,male\nF,null\n".to_vec(),
            ),
        ])));

        let mut named = batch(&[("sex", &[Some("M"), Some("F")])]);
        run(
            &mut named,
            "map_codes",
            json!({"code_map": {"sex": "named.csv"}}),
        );
        assert_eq!(column(&named, "sex"), vec![Some("male".into()), None]);

        let mut positional = batch(&[("sex", &[Some("M"), Some("F")])]);
        run(
            &mut positional,
            "map_codes",
            json!({"code_map": {"sex": "positional.csv"}}),
        );
        assert_eq!(column(&positional, "sex"), vec![Some("male".into()), None]);

        crate::opener::set_opener(previous);
    }

    #[test]
    fn an_explicit_null_mapping_clears_the_value() {
        let mut data = batch(&[("sex", &[Some("M"), Some("F")])]);
        run(
            &mut data,
            "map_codes",
            json!({"code_map": {"sex": {"M": "male", "F": null}}}),
        );
        assert_eq!(column(&data, "sex"), vec![Some("male".into()), None]);
    }

    #[test]
    #[serial_test::serial]
    fn join_data_merges_a_secondary_source() {
        let previous = crate::opener::opener();
        crate::opener::set_opener(std::sync::Arc::new(crate::opener::MemoryOpener::new(vec![
            (
                "addresses.csv".into(),
                b"mrn,city\nm1,Boston\nm3,Denver\n".to_vec(),
            ),
        ])));

        let mut left = batch(&[("mrn", &[Some("m1"), Some("m2")])]);
        run(
            &mut left,
            "join_data",
            json!({
                "secondary_data_source": "addresses.csv",
                "join_type": "left",
                "join_on": "mrn",
                "source_type": "csv"
            }),
        );
        assert_eq!(left.row_count, 2);
        let mut cities = column(&left, "city");
        cities.sort();
        assert_eq!(cities, vec![None, Some("Boston".into())]);

        let mut inner = batch(&[("mrn", &[Some("m1"), Some("m2")])]);
        run(
            &mut inner,
            "join_data",
            json!({
                "secondary_data_source": "addresses.csv",
                "join_type": "inner",
                "join_on": "mrn",
                "source_type": "csv"
            }),
        );
        assert_eq!(inner.row_count, 1);
        assert_eq!(column(&inner, "city"), vec![Some("Boston".into())]);

        let mut outer = batch(&[("mrn", &[Some("m1"), Some("m2")])]);
        run(
            &mut outer,
            "join_data",
            json!({
                "secondary_data_source": "addresses.csv",
                "join_type": "outer",
                "join_on": "mrn"
            }),
        );
        assert_eq!(outer.row_count, 3);

        crate::opener::set_opener(previous);
    }

    #[test]
    #[serial_test::serial]
    fn join_data_reports_a_missing_join_column() {
        let previous = crate::opener::opener();
        crate::opener::set_opener(std::sync::Arc::new(crate::opener::MemoryOpener::new(vec![
            ("addresses.csv".into(), b"other,city\nm1,Boston\n".to_vec()),
        ])));

        let registry = TaskRegistry::new();
        let mut data = batch(&[("mrn", &[Some("m1")])]);
        let params: HashMap<String, Value> = json!({
            "secondary_data_source": "addresses.csv",
            "join_type": "left",
            "join_on": "mrn"
        })
        .as_object()
        .unwrap()
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
        let errors = execute_task_chain(
            &mut data,
            &[crate::contract::Task {
                task: "join_data".into(),
                params,
            }],
            &registry,
        );
        assert_eq!(errors.len(), 1);
        assert_eq!(data.row_count, 1);

        crate::opener::set_opener(previous);
    }

    #[test]
    fn validate_value_replaces_non_matching_values() {
        let mut data = batch(&[("mrn", &[Some("12345"), Some("bad")])]);
        run(
            &mut data,
            "validate_value",
            json!({"column_name": "mrn", "regex": "^[0-9]+$", "no_match_replacement": "invalid"}),
        );
        assert_eq!(
            column(&data, "mrn"),
            vec![Some("12345".into()), Some("invalid".into())]
        );

        let mut dropped = batch(&[("mrn", &[Some("bad")])]);
        run(
            &mut dropped,
            "validate_value",
            json!({"column_name": "mrn", "regex": "^[0-9]+$"}),
        );
        assert_eq!(column(&dropped, "mrn"), vec![None]);
    }
}
