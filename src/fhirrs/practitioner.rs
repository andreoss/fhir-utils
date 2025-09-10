use crate::error::Error;
use crate::fhirrs::common;
use crate::fhirutils::builders;
use crate::fhirutils::constants;
use crate::fhirutils::identifiers;
use serde_json::{json, Map, Value};

pub const FIELDS: &[(&str, &str)] = &[
    ("assigningAuthority", "assigningAuthority"),
    ("practitionerInternalId", "resourceInternalId"),
    ("practitionerNPI", "identifier_practitionerNPI"),
    ("practitionerNameLast", "practitionerNameLast"),
    ("practitionerNameFirst", "practitionerNameFirst"),
    ("practitionerNameText", "practitionerNameText"),
    ("practitionerGender", "practitionerGender"),
    ("practitionerRoleText", "practitionerRoleText"),
    ("practitionerRoleCode", "practitionerRoleCode"),
    ("practitionerRoleCodeList", "practitionerRoleCodeList"),
    ("practitionerRoleCodes", "practitionerRoleCodeList"),
    ("practitionerRoleCodeSystem", "practitionerRoleCodeSystem"),
    ("practitionerRoleCodesSystem", "practitionerRoleCodeSystem"),
    ("practitionerSpecialtyCode", "practitionerSpecialtyCode"),
    (
        "practitionerSpecialtyCodeList",
        "practitionerSpecialtyCodeList",
    ),
    (
        "practitionerSpecialtyCodes",
        "practitionerSpecialtyCodeList",
    ),
    (
        "practitionerSpecialtyCodeSystem",
        "practitionerSpecialtyCodeSystem",
    ),
    (
        "practitionerSpecialtyCodesSystem",
        "practitionerSpecialtyCodeSystem",
    ),
    ("practitionerSpecialtyText", "practitionerSpecialtyText"),
];

pub fn convert_record(
    _group_by_key: &str,
    record: &Value,
    meta: &Value,
) -> Result<Vec<Value>, Error> {
    Ok(build(&common::project(record, FIELDS), meta))
}

pub fn build(record: &Value, meta: &Value) -> Vec<Value> {
    if !contains_practitioner_data(record) && !contains_role_data(record) {
        return Vec::new();
    }

    let id = builders::resource_id(
        builders::field(record, "resourceInternalId")
            .or_else(|| builders::field(record, "identifier_practitionerNPI")),
    );
    let identifier_values = identifiers::identifier_list(record);

    let mut resources = Vec::new();
    let mut practitioner: Option<Value> = None;

    if contains_practitioner_data(record) {
        let mut resource = common::resource("Practitioner", &id, meta);
        if let Some(identifiers) = identifier_values.clone() {
            common::insert_list(&mut resource, "identifier", identifiers);
        }
        if let Some(name) = name(record) {
            common::insert_list(&mut resource, "name", vec![name]);
        }
        common::insert_str(
            &mut resource,
            "gender",
            builders::field(record, "practitionerGender"),
        );
        practitioner = Some(Value::Object(resource));
    }

    if contains_role_data(record) {
        let mut role = common::resource("PractitionerRole", &id, meta);
        if let Some(identifiers) = identifier_values {
            common::insert_list(&mut role, "identifier", identifiers);
        }
        if practitioner.is_some() {
            common::insert(
                &mut role,
                "practitioner",
                builders::reference("Practitioner", &id, None),
            );
        }
        common::insert_list(&mut role, "code", role_concepts(record));
        common::insert_list(&mut role, "specialty", specialty_concepts(record));
        resources.push(Value::Object(role));
    }

    if let Some(practitioner) = practitioner {
        resources.push(practitioner);
    }
    resources
}

pub fn contains_practitioner_data(record: &Value) -> bool {
    let demographics = [
        "practitionerNameLast",
        "practitionerNameFirst",
        "practitionerGender",
    ]
    .iter()
    .any(|field| builders::field(record, field).is_some());
    if demographics {
        return true;
    }
    let identified = builders::field(record, "resourceInternalId").is_some()
        || builders::field(record, "identifier_practitionerNPI").is_some();
    identified && !contains_role_data(record)
}

pub fn contains_role_data(record: &Value) -> bool {
    [
        "practitionerRoleText",
        "practitionerRoleCode",
        "practitionerSpecialtyText",
        "practitionerSpecialtyCode",
    ]
    .iter()
    .any(|field| builders::field(record, field).is_some())
        || !builders::field_list(record, "practitionerRoleCodeList").is_empty()
        || !builders::field_list(record, "practitionerSpecialtyCodeList").is_empty()
}

fn name(record: &Value) -> Option<Value> {
    match builders::field(record, "practitionerNameLast") {
        Some(last) => builders::human_name(
            Some(last),
            builders::field(record, "practitionerNameFirst"),
            None,
            None,
            None,
        ),
        None => {
            let text = builders::field(record, "practitionerNameText")?;
            let mut name = Map::new();
            name.insert("text".into(), json!(text));
            Some(Value::Object(name))
        }
    }
}

fn role_concepts(record: &Value) -> Vec<Value> {
    let list = builders::field_list(record, "practitionerRoleCodeList");
    if !list.is_empty() {
        return list
            .iter()
            .filter_map(|entry| with_id(builders::hl7_codeable_concept(entry), entry_id(entry)))
            .collect();
    }

    let code = builders::field(record, "practitionerRoleCode");
    let text = builders::field(record, "practitionerRoleText");
    let concept = builders::codeable_concept(
        builders::field(record, "practitionerRoleCodeSystem"),
        code,
        None,
        text,
    );
    with_id(concept, code.or(text).map(|value| value.to_string()))
        .into_iter()
        .collect()
}

fn specialty_concepts(record: &Value) -> Vec<Value> {
    let list = builders::field_list(record, "practitionerSpecialtyCodeList");
    if !list.is_empty() {
        return list
            .iter()
            .filter_map(|entry| with_id(builders::hl7_codeable_concept(entry), entry_id(entry)))
            .collect();
    }

    let code = builders::field(record, "practitionerSpecialtyCode");
    let text = builders::field(record, "practitionerSpecialtyText");
    let concept = builders::codeable_concept(
        Some(
            builders::field(record, "practitionerSpecialtyCodeSystem")
                .unwrap_or(constants::PROVIDER_TAXONOMY_SYSTEM),
        ),
        code,
        None,
        text,
    );
    let id = code
        .map(|code| code.chars().take(64).collect::<String>())
        .or_else(|| text.map(|text| text.to_string()));
    with_id(concept, id).into_iter().collect()
}

fn entry_id(entry: &str) -> Option<String> {
    let parts: Vec<&str> = entry.split('^').collect();
    for index in [0, 1, 3] {
        if let Some(value) = parts.get(index).copied().filter(|value| !value.is_empty()) {
            return Some(value.to_string());
        }
    }
    None
}

fn with_id(concept: Option<Value>, id: Option<String>) -> Option<Value> {
    let mut concept = concept?;
    if let Some(id) = id.as_deref().and_then(builders::format_id) {
        if let Some(object) = concept.as_object_mut() {
            object.insert("id".into(), json!(id));
        }
    }
    Some(concept)
}

#[cfg(test)]
mod tests {
    use crate::fhirrs::practitioner::convert_record;
    use crate::fhirrs::testing::meta;
    use crate::fhirutils::constants;
    use serde_json::json;

    #[test]
    fn no_practitioner_data_produces_no_resource() {
        let record = json!({"patientInternalId": "p1"});
        assert!(convert_record("g1", &record, &meta()).unwrap().is_empty());
    }

    #[test]
    fn practitioner_only_when_no_role_data() {
        let record = json!({
            "practitionerInternalId": "pr1",
            "practitionerNPI": "npi1",
            "practitionerNameLast": "House",
            "practitionerNameFirst": "Greg",
            "practitionerGender": "male",
            "assigningAuthority": "authority"
        });
        let resources = convert_record("g1", &record, &meta()).unwrap();
        assert_eq!(resources.len(), 1);
        let practitioner = &resources[0];
        assert_eq!(practitioner["resourceType"], json!("Practitioner"));
        assert_eq!(practitioner["id"], json!("pr1"));
        assert_eq!(practitioner["name"][0]["text"], json!("Greg House"));
        assert_eq!(practitioner["gender"], json!("male"));

        let npi = practitioner["identifier"]
            .as_array()
            .unwrap()
            .iter()
            .find(|identifier| identifier["type"]["coding"][0]["code"] == json!("NPI"))
            .unwrap();
        assert_eq!(npi["value"], json!("npi1"));
    }

    #[test]
    fn role_and_practitioner_are_linked() {
        let record = json!({
            "practitionerInternalId": "pr1",
            "practitionerNameLast": "House",
            "practitionerRoleCode": "doctor",
            "practitionerRoleText": "Attending",
            "practitionerSpecialtyCode": "207R00000X"
        });
        let resources = convert_record("g1", &record, &meta()).unwrap();
        assert_eq!(resources.len(), 2);

        let role = &resources[0];
        assert_eq!(role["resourceType"], json!("PractitionerRole"));
        assert_eq!(
            role["practitioner"],
            json!({"reference": "Practitioner/pr1"})
        );
        assert_eq!(role["code"][0]["id"], json!("doctor"));
        assert_eq!(role["code"][0]["text"], json!("Attending"));
        assert_eq!(
            role["specialty"][0]["coding"][0]["system"],
            json!(constants::PROVIDER_TAXONOMY_SYSTEM)
        );
        assert_eq!(resources[1]["resourceType"], json!("Practitioner"));
    }

    #[test]
    fn role_lists_build_one_concept_per_entry() {
        let record = json!({
            "practitionerInternalId": "pr1",
            "practitionerRoleCodes": ["a^Alpha^SNOMED", "b^Beta"],
            "practitionerSpecialtyCodes": ["s1^Cardiology"]
        });
        let resources = convert_record("g1", &record, &meta()).unwrap();
        assert_eq!(resources.len(), 1);
        let role = &resources[0];
        assert_eq!(role["code"].as_array().unwrap().len(), 2);
        assert_eq!(role["code"][0]["id"], json!("a"));
        assert_eq!(role["code"][1]["coding"][0]["display"], json!("Beta"));
        assert_eq!(role["specialty"][0]["id"], json!("s1"));
    }

    #[test]
    fn practitioner_name_text_is_used_without_a_last_name() {
        let record = json!({
            "practitionerInternalId": "pr1",
            "practitionerNameText": "Dr Who",
            "practitionerGender": "unknown"
        });
        let practitioner = convert_record("g1", &record, &meta()).unwrap().remove(0);
        assert_eq!(practitioner["name"][0]["text"], json!("Dr Who"));
    }
}
