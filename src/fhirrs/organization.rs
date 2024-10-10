use crate::error::Error;
use crate::fhirrs::common;
use crate::fhirutils::builders;
use crate::fhirutils::identifiers;
use serde_json::Value;

pub const FIELDS: &[(&str, &str)] = &[
    ("assigningAuthority", "assigningAuthority"),
    ("organizationResourceInternalId", "resourceInternalId"),
    ("organizationName", "organizationName"),
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
    let name = builders::field(record, "organizationName")?;
    let mut organization = common::resource(
        "Organization",
        &builders::resource_id(builders::field(record, "resourceInternalId")),
        meta,
    );
    if let Some(identifiers) = identifiers::identifier_list(record) {
        common::insert_list(&mut organization, "identifier", identifiers);
    }
    common::insert_str(&mut organization, "name", Some(name));
    Some(Value::Object(organization))
}

#[cfg(test)]
mod tests {
    use crate::fhirrs::organization::convert_record;
    use crate::fhirrs::testing::meta;
    use serde_json::json;

    #[test]
    fn no_name_produces_no_resource() {
        let record = json!({"organizationResourceInternalId": "o1"});
        assert!(convert_record("g1", &record, &meta()).unwrap().is_empty());
    }

    #[test]
    fn organization_carries_name_and_identifier() {
        let record = json!({
            "organizationResourceInternalId": "o1",
            "organizationName": "Vaccine Co",
            "assigningAuthority": "authority"
        });
        let organization = convert_record("g1", &record, &meta()).unwrap().remove(0);
        assert_eq!(organization["resourceType"], json!("Organization"));
        assert_eq!(organization["id"], json!("o1"));
        assert_eq!(organization["name"], json!("Vaccine Co"));
        assert_eq!(organization["identifier"][0]["value"], json!("o1"));
    }
}
