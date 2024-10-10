use crate::error::Error;
use crate::fhirrs::common;
use crate::fhirutils::builders;
use crate::fhirutils::constants;
use crate::fhirutils::identifiers;
use serde_json::Value;

pub const FIELDS: &[(&str, &str)] = &[
    ("assigningAuthority", "assigningAuthority"),
    ("locationResourceInternalId", "resourceInternalId"),
    ("locationName", "locationName"),
    ("locationTypeCode", "locationTypeCode"),
    ("locationTypeText", "locationTypeText"),
    ("locationTypeCodeSystem", "locationTypeCodeSystem"),
];

pub fn convert_record(
    _group_by_key: &str,
    record: &Value,
    meta: &Value,
) -> Result<Vec<Value>, Error> {
    let record = common::project(record, FIELDS);
    Ok(build(&record, meta).into_iter().collect())
}

pub fn build(record: &Value, meta: &Value) -> Option<Value> {
    let name = builders::field(record, "locationName");
    let type_code = builders::field(record, "locationTypeCode");
    let type_text = builders::field(record, "locationTypeText");
    let internal_id = builders::field(record, "resourceInternalId");
    if internal_id.is_none() && name.is_none() && type_code.is_none() && type_text.is_none() {
        return None;
    }

    let mut location = common::resource("Location", &builders::resource_id(internal_id), meta);
    if let Some(identifiers) = identifiers::identifier_list(record) {
        common::insert_list(&mut location, "identifier", identifiers);
    }
    common::insert_str(&mut location, "name", name);

    let concept = builders::codeable_concept(
        Some(
            builders::field(record, "locationTypeCodeSystem")
                .unwrap_or(constants::LOCATION_TYPE_SYSTEM),
        ),
        type_code,
        type_code.and_then(|code| constants::display(constants::LOCATION_TYPE_DISPLAY, code)),
        type_text,
    );
    if let Some(concept) = concept {
        common::insert_list(&mut location, "type", vec![concept]);
    }

    Some(Value::Object(location))
}

#[cfg(test)]
mod tests {
    use crate::fhirrs::location::convert_record;
    use crate::fhirrs::testing::meta;
    use crate::fhirutils::constants;
    use serde_json::json;

    #[test]
    fn no_location_data_produces_no_resource() {
        let record = json!({"patientInternalId": "p1", "mrn": "m1"});
        assert!(convert_record("g1", &record, &meta()).unwrap().is_empty());
    }

    #[test]
    fn location_carries_name_type_and_identifier() {
        let record = json!({
            "locationResourceInternalId": "l1",
            "locationName": "Main ER",
            "locationTypeCode": "ER",
            "assigningAuthority": "authority",
            "patientInternalId": "p1"
        });
        let location = convert_record("g1", &record, &meta()).unwrap().remove(0);

        assert_eq!(location["resourceType"], json!("Location"));
        assert_eq!(location["id"], json!("l1"));
        assert_eq!(location["name"], json!("Main ER"));
        assert_eq!(
            location["type"][0]["coding"][0]["system"],
            json!(constants::LOCATION_TYPE_SYSTEM)
        );
        assert_eq!(
            location["type"][0]["coding"][0]["display"],
            json!("Emergency room")
        );
        assert_eq!(location["identifier"].as_array().unwrap().len(), 1);
        assert_eq!(
            location["identifier"][0]["type"]["coding"][0]["code"],
            json!("RI")
        );
    }
}
