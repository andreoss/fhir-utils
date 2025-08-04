use crate::error::Error;
use crate::fhirutils::builders;
use serde_json::Value;

pub type ConvertFn = fn(&str, &Value, &Value) -> Result<Vec<Value>, Error>;

pub const RESOURCE_KEYS: &[(&str, ConvertFn)] = &[
    ("Patient", crate::fhirrs::patient::convert_record),
    (
        "AllergyIntolerance",
        crate::fhirrs::allergy_intolerance::convert_record,
    ),
    ("Condition", crate::fhirrs::condition::convert_record),
    ("Encounter", crate::fhirrs::encounter::convert_record),
    ("Immunization", not_implemented),
    ("Observation", not_implemented),
    ("Location", crate::fhirrs::location::convert_record),
    ("Organization", crate::fhirrs::organization::convert_record),
    ("Practitioner", crate::fhirrs::practitioner::convert_record),
    ("Procedure", not_implemented),
    ("MedicationUse", not_implemented),
    ("MedicationAdministration", not_implemented),
    ("MedicationRequest", not_implemented),
    ("MedicationStatement", not_implemented),
    ("DocumentReference", not_implemented),
    ("DiagnosticReport", not_implemented),
    ("Unstructured", not_implemented),
    ("Basic", not_implemented),
];

const SOURCE_RECORD_ID_SUFFIX: &str = "SourceRecordId";

pub fn converter_for(resource_type: &str) -> Option<ConvertFn> {
    RESOURCE_KEYS
        .iter()
        .find(|(key, _)| *key == resource_type)
        .map(|(_, converter)| *converter)
}

pub fn convert_record(
    group_by_key: &str,
    record: &Value,
    meta: &Value,
) -> Result<Vec<Value>, Error> {
    let resource_type = builders::field(record, "configResourceType")
        .ok_or_else(|| Error::UnknownResourceType("missing configResourceType".into()))?;
    let converter = converter_for(resource_type)
        .ok_or_else(|| Error::UnknownResourceType(resource_type.to_string()))?;
    converter(group_by_key, record, meta)
}

pub fn source_record_id(record: &Value) -> Option<&str> {
    let object = record.as_object()?;
    let mut keys: Vec<&String> = object
        .keys()
        .filter(|key| key.ends_with(SOURCE_RECORD_ID_SUFFIX))
        .collect();
    keys.sort();
    keys.into_iter()
        .find_map(|key| builders::field(record, key))
}

fn not_implemented(
    _group_by_key: &str,
    record: &Value,
    _meta: &Value,
) -> Result<Vec<Value>, Error> {
    let resource_type = builders::field(record, "configResourceType").unwrap_or("resource");
    Err(Error::Conversion(format!(
        "{resource_type} conversion not implemented"
    )))
}

#[cfg(test)]
mod tests {
    use crate::fhirrs::dispatch::*;
    use serde_json::json;

    const KEYS: &[&str] = &[
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

    #[test]
    fn every_resource_key_has_a_converter() {
        for key in KEYS {
            assert!(converter_for(key).is_some(), "missing converter for {key}");
        }
        assert_eq!(RESOURCE_KEYS.len(), KEYS.len());
        assert!(converter_for("Nonsense").is_none());
    }

    #[test]
    fn convert_record_rejects_an_unknown_resource_type() {
        let record = json!({"configResourceType": "Nonsense"});
        let error = convert_record("key", &record, &json!({})).unwrap_err();
        assert!(error.to_string().contains("Nonsense"));

        let missing = json!({});
        assert!(convert_record("key", &missing, &json!({})).is_err());
    }

    #[test]
    fn source_record_id_is_read_from_the_record() {
        let record = json!({"allergySourceRecordId": "a1", "patientInternalId": "p1"});
        assert_eq!(source_record_id(&record), Some("a1"));
        assert_eq!(source_record_id(&json!({"patientInternalId": "p1"})), None);
    }
}
