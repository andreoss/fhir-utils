use crate::error::Error;
use crate::fhirrs::common;
use crate::fhirutils::builders;
use crate::fhirutils::constants;
use serde_json::{json, Map, Value};

const BASIC_TYPE_SYSTEM: &str = "urn:id:basic-resource-type";
const PATIENT_TOKENS: &str = "patient-tokens";
const TOKEN_TYPE_CODE: &str = "TKN";
const TOKEN_TYPE_DISPLAY: &str = "Token identifier";

pub fn convert_record(
    _group_by_key: &str,
    record: &Value,
    meta: &Value,
) -> Result<Vec<Value>, Error> {
    let patient_id = builders::field(record, "patientInternalIdentifier");
    let id = match patient_id {
        Some(patient_id) => builders::resource_id(Some(&format!("{PATIENT_TOKENS}.{patient_id}"))),
        None => builders::resource_id(None),
    };
    let mut basic = common::resource("Basic", &id, meta);

    common::insert(
        &mut basic,
        "code",
        builders::codeable_concept(
            Some(BASIC_TYPE_SYSTEM),
            Some(PATIENT_TOKENS),
            Some("Patient Tokens"),
            Some("Patient Tokens"),
        ),
    );

    let base_system = builders::field(record, "baseSystem").unwrap_or_default();
    let mut identifier_values = Vec::new();
    for token in builders::field_list(record, "tokenList") {
        if let Some(identifier) = token_identifier(base_system, &token) {
            identifier_values.push(identifier);
        }
    }
    for other in builders::field_list(record, "otherIdentifierList") {
        if let Some(identifier) = other_identifier(base_system, &other) {
            identifier_values.push(identifier);
        }
    }
    common::insert_list(&mut basic, "identifier", identifier_values);

    if let Some(patient_id) = patient_id {
        common::insert(
            &mut basic,
            "subject",
            builders::reference("Patient", patient_id, None),
        );
    }
    if let Some(created) = builders::field(record, "created_date").and_then(builders::date) {
        basic.insert("created".into(), json!(created));
    }

    Ok(vec![Value::Object(basic)])
}

fn parts(entry: &str) -> Option<(&str, &str, Option<&str>)> {
    let fields: Vec<&str> = entry.split('^').collect();
    let name = builders::non_empty(fields.first().copied())?;
    let value = builders::non_empty(fields.get(1).copied())?;
    if value == "null" {
        return None;
    }
    Some((name, value, builders::non_empty(fields.get(2).copied())))
}

fn token_identifier(base_system: &str, entry: &str) -> Option<Value> {
    let (name, value, system) = parts(entry)?;
    let mut identifier = Map::new();
    identifier.insert("id".into(), json!(format!("{base_system}.{name}")));
    identifier.insert("value".into(), json!(value));
    if let Some(system) = system.and_then(builders::uri_format) {
        identifier.insert("system".into(), json!(system));
    }
    common::insert(
        &mut identifier,
        "type",
        builders::codeable_concept(
            Some(constants::IDENTIFIER_TYPE_SYSTEM_RXN),
            Some(TOKEN_TYPE_CODE),
            Some(TOKEN_TYPE_DISPLAY),
            Some(TOKEN_TYPE_DISPLAY),
        ),
    );
    Some(Value::Object(identifier))
}

fn other_identifier(base_system: &str, entry: &str) -> Option<Value> {
    let (name, value, system) = parts(entry)?;
    let mut identifier = Map::new();
    identifier.insert("id".into(), json!(format!("{base_system}.{name}")));
    identifier.insert("value".into(), json!(value));
    if let Some(system) = system.and_then(builders::uri_format) {
        identifier.insert("system".into(), json!(system.clone()));
        if let Some(coding) = builders::coding(Some(name), Some(&system), None) {
            identifier.insert("type".into(), json!({"coding": [coding]}));
        }
    }
    Some(Value::Object(identifier))
}

#[cfg(test)]
mod tests {
    use crate::fhirrs::basic::convert_record;
    use serde_json::{json, Value};

    fn meta() -> Value {
        json!({"extension": []})
    }

    #[test]
    fn basic_carries_tokens_subject_and_created_date() {
        let record = json!({
            "baseSystem": "tokens",
            "patientInternalIdentifier": "p1",
            "created_date": "2021-06-01 10:00:00",
            "tokenList": ["token1^abc^urn:id:token-system", "token2^null^urn:id:token-system"],
            "otherIdentifierList": ["other1^xyz^othersystem"]
        });

        let basic = convert_record("g1", &record, &meta()).unwrap().remove(0);
        assert_eq!(basic["resourceType"], json!("Basic"));
        assert_eq!(basic["id"], json!("patient-tokens.p1"));
        assert_eq!(basic["subject"], json!({"reference": "Patient/p1"}));
        assert_eq!(basic["created"], json!("2021-06-01"));
        assert_eq!(basic["code"]["coding"][0]["code"], json!("patient-tokens"));

        let identifiers = basic["identifier"].as_array().unwrap();
        assert_eq!(identifiers.len(), 2);
        assert_eq!(identifiers[0]["id"], json!("tokens.token1"));
        assert_eq!(identifiers[0]["value"], json!("abc"));
        assert_eq!(identifiers[0]["type"]["coding"][0]["code"], json!("TKN"));
        assert_eq!(identifiers[1]["id"], json!("tokens.other1"));
        assert_eq!(identifiers[1]["system"], json!("urn:id:othersystem"));
        assert_eq!(identifiers[1]["type"]["coding"][0]["code"], json!("other1"));
    }

    #[test]
    fn basic_without_a_patient_identifier_still_builds() {
        let record = json!({"baseSystem": "tokens", "tokenList": ["t^v^s"]});
        let basic = convert_record("g1", &record, &meta()).unwrap().remove(0);
        assert!(basic.get("subject").is_none());
        assert!(basic["id"].as_str().unwrap().contains('.'));
        assert_eq!(basic["identifier"].as_array().unwrap().len(), 1);
    }
}
