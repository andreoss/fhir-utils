use crate::fhirutils::constants;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::{NaiveDate, NaiveDateTime, SecondsFormat, TimeZone};
use chrono_tz::Tz;
use serde_json::{Map, Value};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const DEFAULT_CONTENT_TYPE: &str = "text/plain";

const DATE_TIME_FORMATS: &[&str] = &[
    "%Y-%m-%dT%H:%M:%S",
    "%Y-%m-%d %H:%M:%S",
    "%Y-%m-%dT%H:%M",
    "%Y-%m-%d %H:%M",
    "%m/%d/%Y %H:%M:%S",
    "%Y%m%d%H%M%S",
];

const DATE_FORMATS: &[&str] = &["%Y-%m-%d", "%m/%d/%Y", "%Y%m%d"];

pub fn format_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(
        trimmed
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                    c
                } else {
                    '-'
                }
            })
            .collect(),
    )
}

pub fn resource_id(value: Option<&str>) -> String {
    match value.and_then(format_id) {
        Some(id) => id,
        None => {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default();
            format!("{nanos}.{}", Uuid::new_v4().simple())
        }
    }
}

pub fn uri_format(value: &str) -> Option<String> {
    if value.is_empty() {
        return None;
    }
    if has_scheme(value) {
        Some(value.to_string())
    } else {
        Some(format!("{}{value}", constants::LOCAL_BASE))
    }
}

fn has_scheme(value: &str) -> bool {
    match value.split_once(':') {
        Some((scheme, _)) => {
            !scheme.is_empty()
                && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
        }
        None => false,
    }
}

pub fn code_system(value: &str) -> Option<String> {
    if value.is_empty() {
        return None;
    }
    match constants::system_url(value) {
        Some(url) => Some(url.to_string()),
        None => uri_format(value),
    }
}

pub fn coding(code: Option<&str>, system: Option<&str>, display: Option<&str>) -> Option<Value> {
    let code = non_empty(code);
    let display = non_empty(display);
    let system = non_empty(system).and_then(code_system);
    if code.is_none() && display.is_none() && system.is_none() {
        return None;
    }
    let mut value = Map::new();
    if let Some(system) = system {
        value.insert("system".into(), Value::String(system));
    }
    if let Some(code) = code {
        value.insert("code".into(), Value::String(code.to_string()));
    }
    if let Some(display) = display {
        value.insert("display".into(), Value::String(display.to_string()));
    }
    Some(Value::Object(value))
}

pub fn codeable_concept(
    system: Option<&str>,
    code: Option<&str>,
    display: Option<&str>,
    text: Option<&str>,
) -> Option<Value> {
    let mut concept = codeable_concept_no_text_default(system, code, display, text)?;
    if text.is_none() {
        if let Some(display) = non_empty(display) {
            concept
                .as_object_mut()?
                .insert("text".into(), Value::String(display.to_string()));
        }
    }
    Some(concept)
}

pub fn codeable_concept_no_text_default(
    system: Option<&str>,
    code: Option<&str>,
    display: Option<&str>,
    text: Option<&str>,
) -> Option<Value> {
    let code = non_empty(code);
    let display = non_empty(display);
    let text = non_empty(text);
    let mut concept = Map::new();

    if code.is_some() || display.is_some() {
        if let Some(coding) = coding(code, system, display) {
            concept.insert("coding".into(), Value::Array(vec![coding]));
        }
    }
    if let Some(text) = text {
        concept.insert("text".into(), Value::String(text.to_string()));
    }
    if concept.is_empty() {
        None
    } else {
        Some(Value::Object(concept))
    }
}

pub fn hl7_codeable_concept(value: &str) -> Option<Value> {
    let parts: Vec<&str> = value.split('^').collect();
    let code = parts.first().copied();
    let display = parts.get(1).copied();
    let system = parts.get(2).copied();
    let text = parts.get(3).copied();
    codeable_concept(system, code, display, text)
}

pub fn add_hl7_coded_list(
    concept: Option<Value>,
    entries: &[String],
    custom_system: Option<&str>,
) -> Option<Value> {
    let mut concept = concept;
    for entry in entries {
        let parts: Vec<&str> = entry.split('^').collect();
        let code = parts.first().copied();
        let display = parts.get(1).copied();
        let system = match non_empty(parts.get(2).copied()) {
            Some(system) => Some(system),
            None => custom_system,
        };
        concept = add_codeable_concept(concept, system, code, display);
    }
    concept
}

pub fn add_codeable_concept(
    concept: Option<Value>,
    system: Option<&str>,
    code: Option<&str>,
    display: Option<&str>,
) -> Option<Value> {
    let mut concept = match concept {
        Some(concept) => concept,
        None => return codeable_concept_no_text_default(system, code, display, None),
    };

    if let Some(new_coding) = coding(code, system, display) {
        let object = concept.as_object_mut()?;
        let codings = object
            .entry("coding")
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(codings) = codings.as_array_mut() {
            if !codings.contains(&new_coding) {
                codings.push(new_coding);
            }
        }
    }
    Some(concept)
}

pub fn datetime(value: &str, zone: Option<&str>) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(value) {
        return Some(parsed.to_rfc3339_opts(SecondsFormat::Secs, false));
    }

    let naive = parse_naive(value)?;
    let zone = zone.and_then(|zone| Tz::from_str(zone).ok());
    match zone {
        Some(zone) => zone
            .from_local_datetime(&naive)
            .earliest()
            .map(|value| value.to_rfc3339_opts(SecondsFormat::Secs, false)),
        None => Some(
            chrono::Utc
                .from_utc_datetime(&naive)
                .to_rfc3339_opts(SecondsFormat::Secs, false),
        ),
    }
}

fn parse_naive(value: &str) -> Option<NaiveDateTime> {
    for format in DATE_TIME_FORMATS {
        if let Ok(parsed) = NaiveDateTime::parse_from_str(value, format) {
            return Some(parsed);
        }
    }
    parse_naive_date(value).map(|date| date.into())
}

fn parse_naive_date(value: &str) -> Option<NaiveDate> {
    for format in DATE_FORMATS {
        if let Ok(parsed) = NaiveDate::parse_from_str(value, format) {
            return Some(parsed);
        }
    }
    None
}

pub fn date(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Some(date) = parse_naive_date(value) {
        return Some(date.format("%Y-%m-%d").to_string());
    }
    if let Some(parsed) = parse_naive(value) {
        return Some(parsed.date().format("%Y-%m-%d").to_string());
    }
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|parsed| parsed.date_naive().format("%Y-%m-%d").to_string())
}

pub fn period(start: Option<&str>, end: Option<&str>, zone: Option<&str>) -> Option<Value> {
    let start = non_empty(start).and_then(|value| datetime(value, zone));
    let end = non_empty(end).and_then(|value| datetime(value, zone));
    if start.is_none() && end.is_none() {
        return None;
    }
    let mut value = Map::new();
    if let Some(start) = start {
        value.insert("start".into(), Value::String(start));
    }
    if let Some(end) = end {
        value.insert("end".into(), Value::String(end));
    }
    Some(Value::Object(value))
}

pub fn is_decimal(value: &str) -> bool {
    value.trim().parse::<f64>().is_ok()
}

pub fn quantity(value: Option<&str>, unit: Option<&str>) -> Option<Value> {
    let parsed = non_empty(value)?.trim().parse::<f64>().ok()?;
    let mut quantity = Map::new();
    quantity.insert("value".into(), serde_json::json!(parsed));
    if let Some(unit) = non_empty(unit) {
        quantity.insert("unit".into(), Value::String(unit.to_string()));
    }
    Some(Value::Object(quantity))
}

pub fn duration(value: Option<&str>, unit: Option<&str>) -> Option<Value> {
    quantity(value, unit)
}

#[allow(clippy::too_many_arguments)]
pub fn address(
    address1: Option<&str>,
    address2: Option<&str>,
    city: Option<&str>,
    state: Option<&str>,
    postal_code: Option<&str>,
    country: Option<&str>,
    text: Option<&str>,
) -> Option<Value> {
    let fields = [address1, address2, city, state, postal_code, country, text];
    if fields.iter().all(|field| non_empty(*field).is_none()) {
        return None;
    }

    let mut value = Map::new();
    let mut line = Vec::new();
    if let Some(address1) = non_empty(address1) {
        line.push(Value::String(address1.to_string()));
    }
    if let Some(address2) = non_empty(address2) {
        line.push(Value::String(address2.to_string()));
    }
    if !line.is_empty() {
        value.insert("line".into(), Value::Array(line));
    }
    insert_string(&mut value, "city", city);
    insert_string(&mut value, "state", state);
    insert_string(&mut value, "postalCode", postal_code);
    insert_string(&mut value, "country", country);
    insert_string(&mut value, "text", text);
    Some(Value::Object(value))
}

pub fn contact_point_phone(value: &str) -> Option<Value> {
    let value = non_empty(Some(value))?;
    Some(Value::Array(vec![serde_json::json!({
        "system": "phone",
        "value": value,
    })]))
}

pub fn attachment(
    content_type: Option<&str>,
    data: Option<&str>,
    title: Option<&str>,
    creation: Option<&str>,
    zone: Option<&str>,
) -> Option<Value> {
    let data = non_empty(data)?;
    let mut value = Map::new();
    value.insert(
        "contentType".into(),
        Value::String(
            non_empty(content_type)
                .unwrap_or(DEFAULT_CONTENT_TYPE)
                .to_string(),
        ),
    );
    value.insert("data".into(), Value::String(STANDARD.encode(data)));
    insert_string(&mut value, "title", title);
    if let Some(creation) = non_empty(creation).and_then(|value| datetime(value, zone)) {
        value.insert("creation".into(), Value::String(creation));
    }
    Some(Value::Object(value))
}

pub fn human_name(
    last: Option<&str>,
    first: Option<&str>,
    middle: Option<&str>,
    prefix: Option<&str>,
    suffix: Option<&str>,
) -> Option<Value> {
    let last = non_empty(last)?;
    let given: Vec<&str> = [first, middle]
        .iter()
        .filter_map(|part| non_empty(*part))
        .collect();
    Some(build_name(last, &given, prefix, suffix))
}

pub fn human_name_from_text(
    value: &str,
    prefix: Option<&str>,
    suffix: Option<&str>,
) -> Option<Value> {
    let mut parts: Vec<&str> = value
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .filter(|part| !part.is_empty())
        .collect();
    let last = parts.pop()?;
    Some(build_name(last, &parts, prefix, suffix))
}

fn build_name(last: &str, given: &[&str], prefix: Option<&str>, suffix: Option<&str>) -> Value {
    let mut value = Map::new();
    value.insert("family".into(), Value::String(last.to_string()));
    if !given.is_empty() {
        value.insert(
            "given".into(),
            Value::Array(
                given
                    .iter()
                    .map(|part| Value::String(part.to_string()))
                    .collect(),
            ),
        );
    }
    if let Some(prefix) = non_empty(prefix) {
        value.insert(
            "prefix".into(),
            Value::Array(vec![Value::String(prefix.to_string())]),
        );
    }
    if let Some(suffix) = non_empty(suffix) {
        value.insert(
            "suffix".into(),
            Value::Array(vec![Value::String(suffix.to_string())]),
        );
    }

    let mut text = String::new();
    if let Some(prefix) = non_empty(prefix) {
        text.push_str(prefix);
        text.push(' ');
    }
    for part in given {
        text.push_str(part);
        text.push(' ');
    }
    text.push_str(last);
    if let Some(suffix) = non_empty(suffix) {
        text.push(' ');
        text.push_str(suffix);
    }
    value.insert("text".into(), Value::String(text));
    Value::Object(value)
}

pub fn reference(resource_type: &str, id: &str, display: Option<&str>) -> Option<Value> {
    if resource_type.is_empty() {
        return None;
    }
    let id = format_id(id)?;
    let mut value = Map::new();
    value.insert(
        "reference".into(),
        Value::String(format!("{resource_type}/{id}")),
    );
    insert_string(&mut value, "display", display);
    Some(Value::Object(value))
}

pub fn extension(url: &str, field: &str, value: Value) -> Option<Value> {
    let empty = match &value {
        Value::Null => true,
        Value::String(text) => text.is_empty(),
        Value::Array(items) => items.is_empty(),
        Value::Object(entries) => entries.is_empty(),
        _ => false,
    };
    if empty {
        return None;
    }
    let mut result = Map::new();
    result.insert("url".into(), Value::String(url.to_string()));
    result.insert(field.to_string(), value);
    Some(Value::Object(result))
}

pub fn extension_codeable_concept(
    url: &str,
    code: Option<&str>,
    system: Option<&str>,
    display: Option<&str>,
    text: Option<&str>,
) -> Option<Value> {
    if non_empty(code).is_none() && non_empty(text).is_none() {
        return None;
    }
    let concept = codeable_concept(system, code, display, text)?;
    extension(url, "valueCodeableConcept", concept)
}

pub fn boolean_value(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "t" => Some(true),
        "false" | "f" => Some(false),
        _ => None,
    }
}

pub fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}

fn insert_string(target: &mut Map<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = non_empty(value) {
        target.insert(key.to_string(), Value::String(value.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use crate::fhirutils::builders::*;
    use crate::fhirutils::constants;
    use serde_json::json;

    #[test]
    fn uri_format_prefixes_schemeless_values() {
        assert_eq!(uri_format("http://x/y").unwrap(), "http://x/y");
        assert_eq!(uri_format("urn:id:abc").unwrap(), "urn:id:abc");
        assert_eq!(uri_format("abc").unwrap(), "urn:id:abc");
        assert!(uri_format("").is_none());
    }

    #[test]
    fn code_system_resolves_short_names() {
        assert_eq!(code_system("ICD10").unwrap(), constants::ICD10_SYSTEM);
        assert_eq!(code_system("SNOMED").unwrap(), constants::SNOMED_SYSTEM);
        assert_eq!(code_system("http://loinc.org").unwrap(), "http://loinc.org");
        assert_eq!(code_system("local").unwrap(), "urn:id:local");
    }

    #[test]
    fn format_id_sanitizes_to_the_id_datatype() {
        assert_eq!(format_id(" a b/c ").unwrap(), "a-b-c");
        assert_eq!(format_id("ok.id-1").unwrap(), "ok.id-1");
        assert!(format_id("  ").is_none());
    }

    #[test]
    fn resource_id_prefers_the_supplied_id() {
        assert_eq!(resource_id(Some("abc:1")), "abc-1");
        let generated = resource_id(None);
        let (nanos, hex) = generated.split_once('.').unwrap();
        assert!(nanos.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(hex.len(), 32);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(generated, resource_id(None));
    }

    #[test]
    fn coding_needs_at_least_one_field() {
        assert!(coding(None, None, None).is_none());
        assert_eq!(
            coding(Some("A"), Some("SNOMED"), None).unwrap(),
            json!({"code": "A", "system": constants::SNOMED_SYSTEM})
        );
    }

    #[test]
    fn codeable_concept_defaults_text_to_display() {
        let concept =
            codeable_concept(Some("LOINC"), Some("1234-5"), Some("Glucose"), None).unwrap();
        assert_eq!(concept["text"], json!("Glucose"));
        assert_eq!(
            concept["coding"][0]["system"],
            json!(constants::LOINC_SYSTEM)
        );
        assert_eq!(concept["coding"][0]["code"], json!("1234-5"));

        let with_text = codeable_concept(
            Some("LOINC"),
            Some("1234-5"),
            Some("Glucose"),
            Some("Sugar"),
        )
        .unwrap();
        assert_eq!(with_text["text"], json!("Sugar"));

        assert!(codeable_concept(Some("LOINC"), None, None, None).is_none());
        assert_eq!(
            codeable_concept(None, None, None, Some("free text")).unwrap(),
            json!({"text": "free text"})
        );
    }

    #[test]
    fn hl7_codeable_concept_parses_all_segments() {
        let concept = hl7_codeable_concept("E11.9^Diabetes^ICD10^Diabetes text").unwrap();
        assert_eq!(concept["coding"][0]["code"], json!("E11.9"));
        assert_eq!(concept["coding"][0]["display"], json!("Diabetes"));
        assert_eq!(
            concept["coding"][0]["system"],
            json!(constants::ICD10_SYSTEM)
        );
        assert_eq!(concept["text"], json!("Diabetes text"));

        let code_only = hl7_codeable_concept("E11.9").unwrap();
        assert_eq!(code_only["coding"][0]["code"], json!("E11.9"));
        assert!(code_only.get("text").is_none());
        assert!(hl7_codeable_concept("").is_none());
    }

    #[test]
    fn coded_list_appends_unique_codings() {
        let list = vec!["A^Alpha^SNOMED".to_string(), "B^Beta".to_string()];
        let concept = add_hl7_coded_list(None, &list, Some("local")).unwrap();
        assert_eq!(concept["coding"].as_array().unwrap().len(), 2);
        assert_eq!(
            concept["coding"][0]["system"],
            json!(constants::SNOMED_SYSTEM)
        );
        assert_eq!(concept["coding"][1]["system"], json!("urn:id:local"));

        let repeated = add_hl7_coded_list(Some(concept), &list, Some("local")).unwrap();
        assert_eq!(repeated["coding"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn datetime_localizes_naive_values() {
        assert_eq!(
            datetime("2021-06-01 10:00:00", Some("America/New_York")).unwrap(),
            "2021-06-01T10:00:00-04:00"
        );
        assert_eq!(
            datetime("2021-06-01T10:00:00Z", Some("America/New_York")).unwrap(),
            "2021-06-01T10:00:00+00:00"
        );
        assert_eq!(
            datetime("2021-06-01", Some("UTC")).unwrap(),
            "2021-06-01T00:00:00+00:00"
        );
        assert!(datetime("not a date", None).is_none());
    }

    #[test]
    fn date_returns_the_calendar_day() {
        assert_eq!(date("2021-06-01 10:00:00").unwrap(), "2021-06-01");
        assert_eq!(date("06/01/2021").unwrap(), "2021-06-01");
        assert!(date("").is_none());
    }

    #[test]
    fn period_needs_a_bound() {
        assert!(period(None, None, None).is_none());
        let value = period(Some("2021-06-01"), Some("2021-06-02"), Some("UTC")).unwrap();
        assert_eq!(value["start"], json!("2021-06-01T00:00:00+00:00"));
        assert_eq!(value["end"], json!("2021-06-02T00:00:00+00:00"));
    }

    #[test]
    fn quantity_requires_a_number() {
        assert_eq!(
            quantity(Some("5.5"), Some("mg")).unwrap(),
            json!({"value": 5.5, "unit": "mg"})
        );
        assert!(quantity(Some("many"), Some("mg")).is_none());
        assert!(quantity(None, Some("mg")).is_none());
    }

    #[test]
    fn address_collects_lines() {
        assert!(address(None, None, None, None, None, None, None).is_none());
        let value = address(
            Some("1 Main St"),
            Some("Apt 2"),
            Some("Boston"),
            Some("MA"),
            Some("02101"),
            None,
            None,
        )
        .unwrap();
        assert_eq!(value["line"], json!(["1 Main St", "Apt 2"]));
        assert_eq!(value["city"], json!("Boston"));
        assert_eq!(value["postalCode"], json!("02101"));
        assert!(value.get("country").is_none());
    }

    #[test]
    fn attachment_encodes_content() {
        assert!(attachment(None, None, None, None, None).is_none());
        let value = attachment(None, Some("hello"), Some("note"), None, None).unwrap();
        assert_eq!(value["contentType"], json!("text/plain"));
        assert_eq!(value["data"], json!("aGVsbG8="));
        assert_eq!(value["title"], json!("note"));
    }

    #[test]
    fn human_name_builds_text() {
        assert!(human_name(None, Some("Ann"), None, None, None).is_none());
        let value = human_name(
            Some("Smith"),
            Some("Ann"),
            Some("B"),
            Some("Dr"),
            Some("Jr"),
        )
        .unwrap();
        assert_eq!(value["family"], json!("Smith"));
        assert_eq!(value["given"], json!(["Ann", "B"]));
        assert_eq!(value["text"], json!("Dr Ann B Smith Jr"));
    }

    #[test]
    fn human_name_from_text_takes_the_last_token_as_family() {
        let value = human_name_from_text("John Q Public", None, None).unwrap();
        assert_eq!(value["family"], json!("Public"));
        assert_eq!(value["given"], json!(["John", "Q"]));
        assert_eq!(value["text"], json!("John Q Public"));

        let single = human_name_from_text("Cher", None, None).unwrap();
        assert_eq!(single["family"], json!("Cher"));
        assert!(single.get("given").is_none());
    }

    #[test]
    fn reference_formats_the_resource_path() {
        assert_eq!(
            reference("Patient", "abc", None).unwrap(),
            json!({"reference": "Patient/abc"})
        );
        assert_eq!(
            reference("Practitioner", "a b", Some("Dr Who")).unwrap()["reference"],
            json!("Practitioner/a-b")
        );
        assert!(reference("Patient", "", None).is_none());
    }

    #[test]
    fn extensions_skip_empty_values() {
        assert!(extension("urn:id:x", "valueString", json!("")).is_none());
        assert!(extension("urn:id:x", "valueString", json!(null)).is_none());
        assert_eq!(
            extension("urn:id:x", "valueString", json!("v")).unwrap(),
            json!({"url": "urn:id:x", "valueString": "v"})
        );
        let concept =
            extension_codeable_concept("urn:id:x", Some("A"), Some("SNOMED"), None, None).unwrap();
        assert_eq!(concept["url"], json!("urn:id:x"));
        assert_eq!(
            concept["valueCodeableConcept"]["coding"][0]["code"],
            json!("A")
        );
        assert!(extension_codeable_concept("urn:id:x", None, None, None, None).is_none());
    }

    #[test]
    fn boolean_values_are_recognized() {
        assert_eq!(boolean_value("T"), Some(true));
        assert_eq!(boolean_value("false"), Some(false));
        assert_eq!(boolean_value("maybe"), None);
    }

    #[test]
    fn contact_point_phone_is_a_list() {
        assert_eq!(
            contact_point_phone("555").unwrap(),
            json!([{"system": "phone", "value": "555"}])
        );
        assert!(contact_point_phone("").is_none());
    }
}
