use crate::error::Error;
use crate::fhirrs::common;
use crate::fhirrs::encounter;
use crate::fhirrs::practitioner;
use crate::fhirutils::builders;
use crate::fhirutils::constants;
use crate::fhirutils::identifiers;
use serde_json::{json, Map, Value};

pub fn convert_record(
    group_by_key: &str,
    record: &Value,
    meta: &Value,
) -> Result<Vec<Value>, Error> {
    let record = common::normalized(record);
    let code = builders::field(&record, "procedureCode");
    let code_text = builders::field(&record, "procedureCodeText");
    let code_list = builders::field_list(&record, "procedureCodeList");
    if code.is_none() && code_text.is_none() && code_list.is_empty() {
        return Ok(Vec::new());
    }

    let code_concept = builders::add_hl7_coded_list(
        builders::codeable_concept(
            builders::field(&record, "procedureCodeSystem"),
            code,
            builders::field(&record, "procedureCodeDisplay"),
            code_text,
        ),
        &code_list,
        builders::field(&record, "assigningAuthority"),
    );
    let code_concept = match code_concept {
        Some(concept) => concept,
        None => return Ok(Vec::new()),
    };

    let id = builders::resource_id(builders::field(&record, "resourceInternalId"));
    let mut procedure = common::resource("Procedure", &id, meta);

    let mut identifier_values = identifiers::identifier_list(&record).unwrap_or_default();
    if let Some(external) = identifiers::external_identifier(&code_concept, "Procedure", None) {
        identifier_values.push(external);
    }
    common::insert_list(&mut procedure, "identifier", identifier_values);

    let mut resources = Vec::new();
    let mut encounter_index = None;
    let mut performer = None;

    if has_encounter_data(&record) {
        let encounter = encounter::build(group_by_key, &record, meta, &mut resources);
        for resource in &resources {
            match builders::field(resource, "resourceType") {
                Some("PractitionerRole") => performer = Some(resource.clone()),
                Some("Practitioner") if performer.is_none() => performer = Some(resource.clone()),
                _ => {}
            }
        }
        encounter_index = Some(resources.len());
        if let Some(reference) = common::reference_to(&encounter) {
            procedure.insert("encounter".into(), reference);
        }
        resources.push(encounter);
    } else {
        let practitioners = practitioner::convert_record(group_by_key, &record, meta)?;
        performer = practitioners.first().cloned();
        resources.extend(practitioners);
    }

    common::insert_str(
        &mut procedure,
        "status",
        builders::field(&record, "procedureStatus"),
    );
    common::insert(
        &mut procedure,
        "category",
        builders::codeable_concept(
            builders::field(&record, "procedureCategorySystem"),
            builders::field(&record, "procedureCategory"),
            None,
            builders::field(&record, "procedureCategoryText"),
        ),
    );
    common::insert(
        &mut procedure,
        "subject",
        common::subject_reference(&record, group_by_key),
    );
    procedure.insert("code".into(), code_concept);

    if let Some(performed) = builders::field(&record, "procedurePerformedDateTime")
        .and_then(|value| builders::datetime(value, common::zone(&record)))
    {
        procedure.insert("performedDateTime".into(), json!(performed));
    }

    if let Some(performer) = performer.as_ref().and_then(common::reference_to) {
        common::insert_list(
            &mut procedure,
            "performer",
            vec![json!({"actor": performer})],
        );
    }

    for extension in modifier_extensions(&record) {
        common::push_extension(&mut procedure, Some(extension));
    }

    let procedure = Value::Object(procedure);
    if let Some(index) = encounter_index {
        if let Some(mut reference) = builders::reference("Procedure", &id, None) {
            if let Some(object) = reference.as_object_mut() {
                object.insert("id".into(), json!(id));
                if let Some(sequence) = sequence_extension(&record) {
                    object.insert("extension".into(), Value::Array(vec![sequence]));
                }
            }
            if let Some(encounter) = resources[index].as_object_mut() {
                encounter.insert("reasonReference".into(), Value::Array(vec![reference]));
            }
        }
    }

    resources.push(procedure);
    Ok(resources)
}

fn has_encounter_data(record: &Value) -> bool {
    [
        "encounterInternalId",
        "encounterNumber",
        "procedureEncounterSequenceId",
    ]
    .iter()
    .any(|field| builders::field(record, field).is_some())
}

pub fn modifier_list(record: &Value) -> Vec<String> {
    if let Some(Value::Array(_)) = record.get("procedureModifierList") {
        return builders::field_list(record, "procedureModifierList");
    }
    let raw = match builders::field(record, "procedureModifierList") {
        Some(raw) => raw.trim(),
        None => return Vec::new(),
    };
    if raw.is_empty() {
        return Vec::new();
    }
    if raw.chars().all(|c| c.is_alphanumeric()) {
        return raw
            .chars()
            .collect::<Vec<char>>()
            .chunks(2)
            .map(|chunk| chunk.iter().collect())
            .collect();
    }
    raw.split([' ', ',', ';', '\t'])
        .filter(|part| !part.is_empty())
        .map(|part| part.to_string())
        .collect()
}

fn modifier_extensions(record: &Value) -> Vec<Value> {
    let system = builders::field(record, "procedureModifierSystem");
    modifier_list(record)
        .iter()
        .filter_map(|modifier| {
            let concept = builders::codeable_concept(system, Some(modifier), None, None)?;
            let mut extension = Map::new();
            extension.insert("url".into(), json!(constants::EXT_PROCEDURE_MODIFIER));
            extension.insert("valueCodeableConcept".into(), concept);
            Some(Value::Object(extension))
        })
        .collect()
}

fn sequence_extension(record: &Value) -> Option<Value> {
    let sequence = builders::field(record, "procedureEncounterSequenceId")?;
    let sequence = sequence.parse::<u64>().ok()?;
    builders::extension(
        constants::EXT_PROCEDURE_SEQUENCE,
        "valuePositiveInt",
        json!(sequence),
    )
}

#[cfg(test)]
mod tests {
    use crate::fhirrs::procedure::convert_record;
    use crate::fhirrs::testing::meta;
    use crate::fhirutils::constants;
    use serde_json::{json, Value};

    fn find<'a>(resources: &'a [Value], resource_type: &str) -> &'a Value {
        resources
            .iter()
            .find(|resource| resource["resourceType"] == json!(resource_type))
            .unwrap()
    }

    #[test]
    fn no_usable_code_produces_no_resource() {
        let record = json!({"patientInternalId": "p1", "procedureStatus": "completed"});
        assert!(convert_record("g1", &record, &meta()).unwrap().is_empty());
    }

    #[test]
    fn procedure_carries_code_category_and_performer() {
        let record = json!({
            "resourceInternalId": "pr1",
            "patientInternalId": "p1",
            "procedureCode": "0DTJ0ZZ",
            "procedureCodeSystem": "ICD10PCS",
            "procedureCodeText": "Appendectomy",
            "procedureStatus": "completed",
            "procedureCategory": "surgical",
            "procedurePerformedDateTime": "2021-06-01 09:00:00",
            "practitionerInternalId": "d1",
            "practitionerNameLast": "House",
            "timeZone": "UTC"
        });

        let resources = convert_record("g1", &record, &meta()).unwrap();
        let procedure = find(&resources, "Procedure");

        assert_eq!(procedure["id"], json!("pr1"));
        assert_eq!(procedure["status"], json!("completed"));
        assert_eq!(procedure["subject"], json!({"reference": "Patient/p1"}));
        assert_eq!(
            procedure["code"]["coding"][0]["system"],
            json!(constants::ICD10PCS_SYSTEM)
        );
        assert_eq!(
            procedure["performedDateTime"],
            json!("2021-06-01T09:00:00+00:00")
        );
        assert_eq!(
            procedure["performer"][0]["actor"]["reference"],
            json!("Practitioner/d1")
        );

        let ext_id = procedure["identifier"]
            .as_array()
            .unwrap()
            .iter()
            .find(|identifier| identifier["id"] == json!("extID"))
            .unwrap();
        assert_eq!(ext_id["value"], json!("0DTJ0ZZ-ICD10PCS"));
    }

    #[test]
    fn encounter_data_links_the_procedure_back() {
        let record = json!({
            "resourceInternalId": "pr1",
            "encounterInternalId": "e1",
            "procedureCode": "0DTJ0ZZ",
            "procedureEncounterSequenceId": "3"
        });
        let resources = convert_record("g1", &record, &meta()).unwrap();
        let encounter = find(&resources, "Encounter");
        let reference = &encounter["reasonReference"][0];

        assert_eq!(reference["reference"], json!("Procedure/pr1"));
        assert_eq!(reference["id"], json!("pr1"));
        assert_eq!(
            reference["extension"][0]["url"],
            json!(constants::EXT_PROCEDURE_SEQUENCE)
        );
        assert_eq!(reference["extension"][0]["valuePositiveInt"], json!(3));
        assert_eq!(
            find(&resources, "Procedure")["encounter"],
            json!({"reference": "Encounter/e1"})
        );
    }

    #[test]
    fn modifiers_split_into_extensions() {
        let packed = json!({
            "procedureCode": "1",
            "procedureModifierList": "5926",
            "procedureModifierSystem": "CPT"
        });
        let procedure = convert_record("g1", &packed, &meta()).unwrap().remove(0);
        let extensions = procedure["extension"].as_array().unwrap();
        assert_eq!(extensions.len(), 2);
        assert_eq!(
            extensions[0]["valueCodeableConcept"]["coding"][0]["code"],
            json!("59")
        );
        assert_eq!(
            extensions[0]["valueCodeableConcept"]["coding"][0]["system"],
            json!(constants::CPT_SYSTEM)
        );

        let separated = json!({"procedureCode": "1", "procedureModifierList": "59, 26 XU"});
        let procedure = convert_record("g1", &separated, &meta()).unwrap().remove(0);
        assert_eq!(procedure["extension"].as_array().unwrap().len(), 3);

        let listed = json!({"procedureCode": "1", "procedureModifierList": ["59", "26"]});
        let procedure = convert_record("g1", &listed, &meta()).unwrap().remove(0);
        assert_eq!(procedure["extension"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn code_list_only_still_builds_a_procedure() {
        let record = json!({"procedureCodeList": ["0DTJ0ZZ^Appendectomy^ICD10PCS"]});
        let procedure = convert_record("g1", &record, &meta()).unwrap().remove(0);
        assert_eq!(
            procedure["code"]["coding"][0]["display"],
            json!("Appendectomy")
        );
    }
}
