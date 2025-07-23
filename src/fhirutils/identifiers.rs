use crate::fhirutils::builders;
use crate::fhirutils::constants;
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{Map, Value};

pub struct IdentifierType {
    pub code: &'static str,
    pub display: &'static str,
}

pub mod types {
    use super::IdentifierType;

    pub const PATIENT_INTERNAL_ID: IdentifierType = IdentifierType {
        code: "PI",
        display: "Patient internal identifier",
    };
    pub const SOCIAL_SECURITY_NUMBER: IdentifierType = IdentifierType {
        code: "SS",
        display: "Social Security number",
    };
    pub const MEDICAL_RECORD_NUMBER: IdentifierType = IdentifierType {
        code: "MR",
        display: "Medical record number",
    };
    pub const ACCOUNT_NUMBER: IdentifierType = IdentifierType {
        code: "AN",
        display: "Account number",
    };
    pub const ENCOUNTER_NUMBER: IdentifierType = IdentifierType {
        code: "VN",
        display: "Visit number",
    };
    pub const RESOURCE_ID: IdentifierType = IdentifierType {
        code: "RI",
        display: "Resource identifier",
    };
    pub const DRIVERS_LICENSE: IdentifierType = IdentifierType {
        code: "DL",
        display: "Driver's license number",
    };
    pub const NPI: IdentifierType = IdentifierType {
        code: "NPI",
        display: "National provider identifier",
    };
    pub const PRESCRIPTION_NUMBER: IdentifierType = IdentifierType {
        code: "RXN",
        display: "Prescription Number",
    };
}

pub const IDENTIFIER_FIELDS: &[(&str, &IdentifierType)] = &[
    ("patientInternalId", &types::PATIENT_INTERNAL_ID),
    ("ssn", &types::SOCIAL_SECURITY_NUMBER),
    ("mrn", &types::MEDICAL_RECORD_NUMBER),
    ("accountNumber", &types::ACCOUNT_NUMBER),
    ("encounterNumber", &types::ENCOUNTER_NUMBER),
    ("resourceInternalId", &types::RESOURCE_ID),
    ("driversLicense", &types::DRIVERS_LICENSE),
    ("identifier_practitionerNPI", &types::NPI),
    ("medicationRxNumber", &types::PRESCRIPTION_NUMBER),
];

const ID_INCLUDES_VALUE: &[&str] = &["MR", "DL", "AN", "VN", "PI", "RI", "RXN"];

const AUTHORITY_FIELDS: &[(&str, &str)] = &[
    ("PI", "assigningAuthority"),
    ("MR", "assigningAuthority"),
    ("AN", "assigningAuthority"),
    ("RI", "assigningAuthority"),
    ("VN", "assigningAuthority"),
    ("RXN", "assigningAuthority"),
    ("DL", "driversLicenseSystem"),
    ("SS", "ssnSystem"),
];

const EXT_ID_PREFERRED_SYSTEMS: &[(&str, &[&str])] = &[
    (
        "AllergyIntolerance",
        &["SNOMED", "ICD10", "ICD9", "LOINC", "NCI", "MESH", "UMLS"],
    ),
    (
        "Condition",
        &["ICD10", "ICD9", "SNOMED", "LOINC", "NCI", "MESH", "UMLS"],
    ),
    (
        "Immunization",
        &[
            "CVX", "RXNORM", "NDC", "SNOMED", "LOINC", "CPT", "MESH", "NCI", "UMLS",
        ],
    ),
    (
        "Observation",
        &["LOINC", "ICD10", "ICD9", "SNOMED", "MESH", "NCI", "UMLS"],
    ),
    (
        "MedicationRequest",
        &["RXNORM", "NDC", "SNOMED", "MESH", "NCI", "UMLS"],
    ),
    (
        "MedicationAdministration",
        &["RXNORM", "NDC", "SNOMED", "MESH", "NCI", "UMLS"],
    ),
    (
        "MedicationStatement",
        &["RXNORM", "NDC", "SNOMED", "MESH", "NCI", "UMLS"],
    ),
    (
        "Procedure",
        &["CPT", "ICD10PCS", "SNOMED", "NCI", "LOINC", "MESH", "UMLS"],
    ),
];

pub fn normalize_ssn(value: &str) -> Option<String> {
    let digits = value.replace('-', "");
    if digits.len() != 9 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let unique: std::collections::HashSet<char> = digits.chars().collect();
    if unique.len() <= 1 {
        return None;
    }
    Some(digits)
}

pub fn build_identifier(
    value: &str,
    system: Option<&str>,
    type_code_system: Option<&str>,
    type_code: Option<&str>,
    type_text: Option<&str>,
) -> Option<Value> {
    let value = builders::non_empty(Some(value))?;

    let identifier_id = match type_code {
        Some(code) if ID_INCLUDES_VALUE.contains(&code) => {
            builders::format_id(&format!("{code}.{value}"))
        }
        Some(code) => builders::format_id(code),
        None => None,
    };

    let mut identifier = Map::new();
    if let Some(identifier_id) = identifier_id {
        identifier.insert("id".into(), Value::String(identifier_id));
    }
    identifier.insert("value".into(), Value::String(value.to_string()));
    if let Some(system) = builders::non_empty(system).and_then(builders::uri_format) {
        identifier.insert("system".into(), Value::String(system));
    }

    let coding = builders::coding(type_code, type_code_system, type_text);
    if coding.is_some() || builders::non_empty(type_text).is_some() {
        let mut concept = Map::new();
        if let Some(coding) = coding {
            concept.insert("coding".into(), Value::Array(vec![coding]));
        }
        if let Some(text) = builders::non_empty(type_text) {
            concept.insert("text".into(), Value::String(text.to_string()));
        }
        identifier.insert("type".into(), Value::Object(concept));
    }

    Some(Value::Object(identifier))
}

pub fn build_identifier_by_type(
    value: &str,
    system: Option<&str>,
    id_type: &IdentifierType,
) -> Option<Value> {
    let type_system = if id_type.code == types::PRESCRIPTION_NUMBER.code {
        constants::IDENTIFIER_TYPE_SYSTEM_RXN
    } else {
        constants::IDENTIFIER_TYPE_SYSTEM
    };
    build_identifier(
        value,
        system,
        Some(type_system),
        Some(id_type.code),
        Some(id_type.display),
    )
}

pub fn build_extid_identifier(value: &str) -> Option<Value> {
    let value = builders::non_empty(Some(value))?;
    Some(serde_json::json!({
        "id": "extID",
        "value": value,
        "system": constants::EXT_ID_SYSTEM,
    }))
}

pub fn identifier_list(record: &Value) -> Option<Vec<Value>> {
    let mut identifiers = Vec::new();
    for (field, id_type) in IDENTIFIER_FIELDS {
        let value = match builders::field(record, field) {
            Some(value) if value != "None" => value,
            _ => continue,
        };
        let system = AUTHORITY_FIELDS
            .iter()
            .find(|(code, _)| *code == id_type.code)
            .and_then(|(_, system_field)| builders::field(record, system_field));
        if let Some(identifier) = build_identifier_by_type(value, system, id_type) {
            identifiers.push(identifier);
        }
    }
    if identifiers.is_empty() {
        None
    } else {
        Some(identifiers)
    }
}

pub fn external_identifier(
    concept: &Value,
    resource_type: &str,
    timestamp: Option<&str>,
) -> Option<Value> {
    let codings = concept.get("coding").and_then(|value| value.as_array());
    let preferred = preferred_coding(resource_type, codings);

    let mut value = match preferred {
        Some(coding) => coding_value(coding)?,
        None => concept
            .get("text")
            .and_then(|value| value.as_str())
            .map(|text| text.to_string())?,
    };

    if resource_type == "Observation" {
        value = format!("{}-{value}", observation_timestamp(timestamp));
    }

    build_extid_identifier(&value)
}

fn coding_value(coding: &Value) -> Option<String> {
    let code = coding.get("code").and_then(|value| value.as_str());
    let system = coding.get("system").and_then(|value| value.as_str());
    let display = coding.get("display").and_then(|value| value.as_str());

    match (code, system) {
        (Some(code), Some(system)) => {
            let suffix = match constants::system_short_name(system) {
                Some(short) => short.to_string(),
                None => system
                    .strip_prefix(constants::LOCAL_BASE)
                    .unwrap_or(system)
                    .to_string(),
            };
            Some(format!("{code}-{suffix}"))
        }
        (Some(code), None) => Some(code.to_string()),
        (None, _) => display.map(|display| display.to_string()),
    }
}

fn preferred_coding<'a>(resource_type: &str, codings: Option<&'a Vec<Value>>) -> Option<&'a Value> {
    let codings = codings?;
    if codings.is_empty() {
        return None;
    }

    if let Some((_, systems)) = EXT_ID_PREFERRED_SYSTEMS
        .iter()
        .find(|(name, _)| *name == resource_type)
    {
        for preferred in *systems {
            for coding in codings {
                let system = coding.get("system").and_then(|value| value.as_str());
                let matches = system.is_some_and(|system| {
                    system == *preferred || constants::system_short_name(system) == Some(preferred)
                });
                if matches {
                    return Some(coding);
                }
            }
        }
    }

    codings
        .iter()
        .find(|coding| coding.get("system").is_some())
        .or_else(|| codings.first())
}

fn observation_timestamp(timestamp: Option<&str>) -> String {
    let parsed = timestamp
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.to_utc());
    match parsed {
        Some(value) => value.format("%Y%m%d%H%M%S").to_string(),
        None => Utc::now().format("%Y%m%d%H%M%S").to_string(),
    }
}

pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use crate::fhirutils::constants;
    use crate::fhirutils::identifiers::*;
    use serde_json::json;

    #[test]
    fn normalizes_social_security_numbers() {
        assert_eq!(normalize_ssn("123-45-6789").unwrap(), "123456789");
        assert_eq!(normalize_ssn("123456789").unwrap(), "123456789");
        assert!(normalize_ssn("111111111").is_none());
        assert!(normalize_ssn("12345").is_none());
        assert!(normalize_ssn("abcdefghi").is_none());
        assert!(normalize_ssn("").is_none());
    }

    #[test]
    fn builds_identifiers_from_record_fields() {
        let record = json!({
            "patientInternalId": "p1",
            "mrn": "m1",
            "accountNumber": "a1",
            "encounterNumber": "e1",
            "assigningAuthority": "authority",
            "ssn": "123-45-6789",
            "ssnSystem": "http://ssn",
            "driversLicense": "dl1",
            "driversLicenseSystem": "http://dl",
            "identifier_practitionerNPI": "npi1",
            "medicationRxNumber": "rx1"
        });

        let identifiers = identifier_list(&record).unwrap();
        let by_type = |code: &str| {
            identifiers
                .iter()
                .find(|value| value["type"]["coding"][0]["code"] == json!(code))
                .unwrap()
                .clone()
        };

        let patient = by_type("PI");
        assert_eq!(patient["value"], json!("p1"));
        assert_eq!(patient["id"], json!("PI.p1"));
        assert_eq!(patient["system"], json!("urn:id:authority"));
        assert_eq!(
            patient["type"]["coding"][0]["system"],
            json!(constants::IDENTIFIER_TYPE_SYSTEM)
        );
        assert_eq!(
            patient["type"]["coding"][0]["display"],
            json!("Patient internal identifier")
        );

        assert_eq!(by_type("SS")["system"], json!("http://ssn"));
        assert_eq!(by_type("SS")["id"], json!("SS"));
        assert_eq!(by_type("SS")["value"], json!("123-45-6789"));
        assert_eq!(by_type("DL")["system"], json!("http://dl"));
        assert_eq!(by_type("MR")["system"], json!("urn:id:authority"));
        assert_eq!(by_type("AN")["id"], json!("AN.a1"));
        assert_eq!(by_type("VN")["value"], json!("e1"));
        assert_eq!(by_type("NPI")["id"], json!("NPI"));
        assert_eq!(
            by_type("RXN")["type"]["coding"][0]["system"],
            json!(constants::IDENTIFIER_TYPE_SYSTEM_RXN)
        );
    }

    #[test]
    fn skips_missing_and_none_values() {
        let record = json!({"patientInternalId": "p1", "mrn": "None", "ssn": null});
        let identifiers = identifier_list(&record).unwrap();
        assert_eq!(identifiers.len(), 1);
        assert!(identifier_list(&json!({})).is_none());
    }

    #[test]
    fn external_identifier_uses_the_preferred_system() {
        let concept = json!({
            "coding": [
                {"code": "111", "system": constants::SNOMED_SYSTEM},
                {"code": "E11.9", "system": constants::ICD10_SYSTEM}
            ],
            "text": "diabetes"
        });

        let identifier = external_identifier(&concept, "Condition", None).unwrap();
        assert_eq!(identifier["value"], json!("E11.9-ICD10"));
        assert_eq!(identifier["system"], json!(constants::EXT_ID_SYSTEM));
        assert_eq!(identifier["id"], json!("extID"));

        let allergy = external_identifier(&concept, "AllergyIntolerance", None).unwrap();
        assert_eq!(allergy["value"], json!("111-SNOMED"));
    }

    #[test]
    fn external_identifier_falls_back_to_unranked_systems_then_text() {
        let unranked = json!({"coding": [{"code": "X1", "system": "urn:id:local"}]});
        assert_eq!(
            external_identifier(&unranked, "Condition", None).unwrap()["value"],
            json!("X1-local")
        );

        let text_only = json!({"text": "free text"});
        assert_eq!(
            external_identifier(&text_only, "Condition", None).unwrap()["value"],
            json!("free text")
        );

        let no_system = json!({"coding": [{"code": "X2"}]});
        assert_eq!(
            external_identifier(&no_system, "Condition", None).unwrap()["value"],
            json!("X2")
        );

        assert!(external_identifier(&json!({}), "Condition", None).is_none());
    }

    #[test]
    fn observation_external_identifier_is_timestamped() {
        let concept = json!({"coding": [{"code": "1234-5", "system": constants::LOINC_SYSTEM}]});
        let identifier =
            external_identifier(&concept, "Observation", Some("2021-06-01T10:00:00+00:00"))
                .unwrap();
        assert_eq!(identifier["value"], json!("20210601100000-1234-5-LOINC"));

        let generated = external_identifier(&concept, "Observation", None).unwrap();
        let value = generated["value"].as_str().unwrap();
        assert_eq!(value.len(), "20210601100000-1234-5-LOINC".len());
        assert!(value.ends_with("-1234-5-LOINC"));
    }

    #[test]
    fn build_identifier_requires_a_value() {
        assert!(build_identifier_by_type("", None, &types::PATIENT_INTERNAL_ID).is_none());
        let identifier =
            build_identifier_by_type("v1", Some("authority"), &types::RESOURCE_ID).unwrap();
        assert_eq!(identifier["id"], json!("RI.v1"));
        assert_eq!(identifier["type"]["text"], json!("Resource identifier"));
    }
}
