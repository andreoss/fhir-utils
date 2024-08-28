use crate::fhirutils::builders;
use crate::fhirutils::identifiers;
use serde_json::{Map, Value};

pub fn normalized(record: &Value) -> Value {
    let mut record = record.clone();
    if let Some(object) = record.as_object_mut() {
        let ssn = builders::field(&Value::Object(object.clone()), "ssn")
            .and_then(identifiers::normalize_ssn);
        match ssn {
            Some(ssn) => {
                object.insert("ssn".into(), Value::String(ssn));
            }
            None => {
                object.remove("ssn");
            }
        }
    }
    record
}

pub fn zone(record: &Value) -> Option<&str> {
    builders::field(record, "timeZone")
}

pub fn subject_reference(record: &Value, group_by_key: &str) -> Option<Value> {
    let id = builders::field(record, "patientInternalId").unwrap_or(group_by_key);
    builders::reference("Patient", id, None)
}

pub fn encounter_reference(record: &Value) -> Option<Value> {
    let id = builders::field(record, "encounterInternalId")?;
    builders::reference("Encounter", id, None)
}

pub fn human_name(record: &Value) -> Option<Value> {
    let prefix =
        builders::field(record, "namePrefix").or_else(|| builders::field(record, "prefix"));
    let suffix =
        builders::field(record, "nameSuffix").or_else(|| builders::field(record, "suffix"));
    let last = builders::field(record, "nameLast");
    let first = builders::field(record, "nameFirst");
    let middle = builders::field(record, "nameMiddle");
    let first_middle = builders::field(record, "nameFirstMiddle");

    if let (Some(last), None, Some(first_middle)) = (last, first, first_middle) {
        return builders::human_name_from_text(&format!("{first_middle} {last}"), prefix, suffix);
    }
    if let Some(last) = last {
        return builders::human_name(Some(last), first, middle, prefix, suffix);
    }
    let full = builders::field(record, "nameFirstMiddleLast")?;
    builders::human_name_from_text(full, prefix, suffix)
}

pub fn resource(resource_type: &str, id: &str, meta: &Value) -> Map<String, Value> {
    let mut resource = Map::new();
    resource.insert("resourceType".into(), Value::String(resource_type.into()));
    resource.insert("id".into(), Value::String(id.to_string()));
    if meta.is_object() {
        resource.insert("meta".into(), meta.clone());
    }
    resource
}

pub fn insert(target: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        target.insert(key.to_string(), value);
    }
}

pub fn insert_str(target: &mut Map<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = builders::non_empty(value) {
        target.insert(key.to_string(), Value::String(value.to_string()));
    }
}

pub fn insert_list(target: &mut Map<String, Value>, key: &str, values: Vec<Value>) {
    if !values.is_empty() {
        target.insert(key.to_string(), Value::Array(values));
    }
}

pub fn push_extension(target: &mut Map<String, Value>, extension: Option<Value>) {
    if let Some(extension) = extension {
        if let Some(extensions) = target
            .entry("extension")
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
        {
            extensions.push(extension);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::fhirrs::common::*;
    use serde_json::json;

    #[test]
    fn ssn_is_normalized_or_dropped() {
        let valid = normalized(&json!({"ssn": "123-45-6789"}));
        assert_eq!(valid["ssn"], json!("123456789"));
        let invalid = normalized(&json!({"ssn": "999999999", "mrn": "m"}));
        assert!(invalid.get("ssn").is_none());
        assert_eq!(invalid["mrn"], json!("m"));
    }

    #[test]
    fn subject_falls_back_to_the_group_key() {
        assert_eq!(
            subject_reference(&json!({"patientInternalId": "p1"}), "g1").unwrap(),
            json!({"reference": "Patient/p1"})
        );
        assert_eq!(
            subject_reference(&json!({}), "g1").unwrap(),
            json!({"reference": "Patient/g1"})
        );
    }

    #[test]
    fn encounter_reference_needs_an_internal_id() {
        assert_eq!(
            encounter_reference(&json!({"encounterInternalId": "e1"})).unwrap(),
            json!({"reference": "Encounter/e1"})
        );
        assert!(encounter_reference(&json!({})).is_none());
    }

    #[test]
    fn human_name_reads_the_name_fields() {
        let split = human_name(&json!({"nameLast": "Smith", "nameFirst": "Ann"})).unwrap();
        assert_eq!(split["text"], json!("Ann Smith"));

        let first_middle =
            human_name(&json!({"nameLast": "Smith", "nameFirstMiddle": "Ann B"})).unwrap();
        assert_eq!(first_middle["given"], json!(["Ann", "B"]));
        assert_eq!(first_middle["family"], json!("Smith"));

        let full = human_name(&json!({"nameFirstMiddleLast": "Ann B Smith"})).unwrap();
        assert_eq!(full["family"], json!("Smith"));

        let last_only = human_name(&json!({"nameLast": "Smith"})).unwrap();
        assert_eq!(last_only["text"], json!("Smith"));

        assert!(human_name(&json!({"gender": "female"})).is_none());
    }

    #[test]
    fn resource_carries_type_id_and_meta() {
        let built = resource("Patient", "p1", &json!({"extension": []}));
        assert_eq!(built["resourceType"], json!("Patient"));
        assert_eq!(built["id"], json!("p1"));
        assert!(built.contains_key("meta"));
        assert!(!resource("Patient", "p1", &json!(null)).contains_key("meta"));
    }
}
