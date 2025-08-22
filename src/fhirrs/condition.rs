use crate::error::Error;
use crate::fhirrs::common;
use crate::fhirrs::encounter;
use crate::fhirutils::builders;
use crate::fhirutils::constants;
use crate::fhirutils::identifiers;
use serde_json::{json, Map, Value};

const CATEGORY_ENCOUNTER_DIAGNOSIS: &str = "encounter-diagnosis";
const CATEGORY_PROBLEM_LIST: &str = "problem-list-item";

const CHRONICITY_CONCEPTS: &[(&str, &str, &str)] = &[
    ("chronic", "90734009", "Chronic (qualifier value)"),
    ("acute", "373933003", "Acute onset"),
];

pub fn convert_record(
    group_by_key: &str,
    record: &Value,
    meta: &Value,
) -> Result<Vec<Value>, Error> {
    let record = common::normalized(record);
    let code = builders::field(&record, "conditionCode");
    let code_text = builders::field(&record, "conditionCodeText");
    if code.is_none() && code_text.is_none() {
        return Ok(Vec::new());
    }

    let id = builders::resource_id(builders::field(&record, "resourceInternalId"));
    let mut condition = common::resource("Condition", &id, meta);

    let code_concept = builders::codeable_concept(
        Some(builders::field(&record, "conditionCodeSystem").unwrap_or(constants::SNOMED_SYSTEM)),
        code,
        None,
        code_text,
    );

    let mut identifier_values = identifiers::identifier_list(&record).unwrap_or_default();
    if let Some(concept) = &code_concept {
        if let Some(external) = identifiers::external_identifier(concept, "Condition", None) {
            identifier_values.push(external);
        }
    }
    common::insert_list(&mut condition, "identifier", identifier_values);

    let category = category(&record);
    if let Some(concept) = category_concept(category) {
        common::insert_list(&mut condition, "category", vec![concept]);
    }

    let mut resources = Vec::new();
    let mut encounter_index = None;
    if has_encounter_data(&record) {
        let encounter = encounter::build(group_by_key, &record, meta, &mut resources);
        if let Some(reference) = common::reference_to(&encounter) {
            condition.insert("encounter".into(), reference);
        }
        encounter_index = Some(resources.len());
        resources.push(encounter);
    }

    if let Some(status) = builders::field(&record, "conditionClinicalStatus") {
        common::insert(
            &mut condition,
            "clinicalStatus",
            builders::codeable_concept(
                Some(constants::CONDITION_CLINICAL_STATUS_SYSTEM),
                Some(status),
                constants::display(constants::CONDITION_CLINICAL_STATUS_DISPLAY, status),
                None,
            ),
        );
    }
    if let Some(status) = builders::field(&record, "conditionVerificationStatus") {
        common::insert(
            &mut condition,
            "verificationStatus",
            builders::codeable_concept(
                Some(constants::CONDITION_VERIFICATION_STATUS_SYSTEM),
                Some(status),
                constants::display(constants::CONDITION_VERIFICATION_STATUS_DISPLAY, status),
                None,
            ),
        );
    }

    let zone = common::zone(&record);
    for (field, key) in [
        ("conditionRecordedDateTime", "recordedDate"),
        ("conditionOnsetDateTime", "onsetDateTime"),
        ("conditionAbatementDateTime", "abatementDateTime"),
    ] {
        if let Some(value) =
            builders::field(&record, field).and_then(|value| builders::datetime(value, zone))
        {
            condition.insert(key.into(), json!(value));
        }
    }

    common::insert(
        &mut condition,
        "severity",
        builders::codeable_concept(
            Some(
                builders::field(&record, "conditionSeveritySystem")
                    .unwrap_or(constants::SNOMED_SYSTEM),
            ),
            builders::field(&record, "conditionSeverityCode"),
            None,
            builders::field(&record, "conditionSeverityText"),
        ),
    );
    common::insert(
        &mut condition,
        "subject",
        common::subject_reference(&record, group_by_key),
    );
    common::insert(&mut condition, "code", code_concept.clone());
    common::push_extension(&mut condition, chronicity_extension(&record));

    let condition = Value::Object(condition);
    if let Some(index) = encounter_index {
        let reference = condition_reference(&condition, code_concept.as_ref());
        link_encounter(&mut resources[index], &record, category, reference);
    }

    resources.push(condition);
    Ok(resources)
}

fn category(record: &Value) -> Option<&str> {
    match builders::field(record, "conditionCategory") {
        Some(category) => Some(category),
        None => {
            builders::field(record, "conditionDiagnosisRank").map(|_| CATEGORY_ENCOUNTER_DIAGNOSIS)
        }
    }
}

fn category_concept(category: Option<&str>) -> Option<Value> {
    let category = category?;
    if category != CATEGORY_PROBLEM_LIST && category != CATEGORY_ENCOUNTER_DIAGNOSIS {
        return None;
    }
    builders::codeable_concept(
        Some(constants::CONDITION_CATEGORY_SYSTEM),
        Some(category),
        constants::display(constants::CONDITION_CATEGORY_DISPLAY, category),
        None,
    )
}

fn has_encounter_data(record: &Value) -> bool {
    [
        "encounterInternalId",
        "encounterClaimType",
        "conditionDiagnosisUse",
        "conditionDiagnosisRank",
    ]
    .iter()
    .any(|field| builders::field(record, field).is_some())
}

fn chronicity_extension(record: &Value) -> Option<Value> {
    let chronicity = builders::field(record, "conditionChronicity")?;
    let mut concept = Map::new();
    concept.insert("text".into(), json!(chronicity));
    if let Some((_, code, display)) = CHRONICITY_CONCEPTS
        .iter()
        .find(|(name, _, _)| *name == chronicity.to_ascii_lowercase())
    {
        if let Some(coding) =
            builders::coding(Some(code), Some(constants::SNOMED_SYSTEM), Some(display))
        {
            concept.insert("coding".into(), Value::Array(vec![coding]));
        }
    }
    builders::extension(
        constants::EXT_CHRONICITY,
        "valueCodeableConcept",
        Value::Object(concept),
    )
}

fn condition_reference(condition: &Value, code_concept: Option<&Value>) -> Option<Value> {
    let display = code_concept.and_then(|concept| {
        let code = concept
            .get("coding")
            .and_then(|codings| codings.get(0))
            .and_then(|coding| coding.get("code"))
            .and_then(|code| code.as_str());
        let text = concept.get("text").and_then(|text| text.as_str());
        match (code, text) {
            (Some(code), Some(text)) => Some(format!("{code} ({text})")),
            (Some(code), None) => Some(code.to_string()),
            (None, text) => text.map(|text| text.to_string()),
        }
    });
    let id = builders::field(condition, "id")?;
    builders::reference("Condition", id, display.as_deref())
}

fn link_encounter(
    encounter: &mut Value,
    record: &Value,
    category: Option<&str>,
    reference: Option<Value>,
) {
    let reference = match reference {
        Some(reference) => reference,
        None => return,
    };
    let encounter = match encounter.as_object_mut() {
        Some(encounter) => encounter,
        None => return,
    };

    match category {
        Some(CATEGORY_ENCOUNTER_DIAGNOSIS) => {
            let rank = builders::field(record, "conditionDiagnosisRank")
                .and_then(|rank| rank.parse::<i64>().ok());
            let diagnosis_id = builders::field(record, "conditionDiagnosisRank")
                .and_then(builders::format_id)
                .or_else(|| {
                    reference
                        .get("display")
                        .and_then(|display| display.as_str())
                        .and_then(builders::format_id)
                });

            let mut diagnosis = Map::new();
            if let Some(diagnosis_id) = diagnosis_id {
                diagnosis.insert("id".into(), json!(diagnosis_id));
            }
            if let Some(rank) = rank {
                diagnosis.insert("rank".into(), json!(rank));
            }
            if let Some(use_code) = builders::field(record, "conditionDiagnosisUse") {
                common::insert(
                    &mut diagnosis,
                    "use",
                    builders::codeable_concept(
                        Some(constants::EXT_DIAGNOSIS_USE),
                        Some(use_code),
                        constants::display(constants::DIAGNOSIS_USE_DISPLAY, use_code),
                        None,
                    ),
                );
            }
            diagnosis.insert("condition".into(), reference);
            encounter.insert(
                "diagnosis".into(),
                Value::Array(vec![Value::Object(diagnosis)]),
            );
        }
        Some(CATEGORY_PROBLEM_LIST) => {
            let mut reference = reference;
            let id = reference
                .get("reference")
                .and_then(|value| value.as_str())
                .and_then(builders::format_id);
            if let (Some(id), Some(object)) = (id, reference.as_object_mut()) {
                object.insert("id".into(), json!(id));
            }
            encounter.insert("reasonReference".into(), Value::Array(vec![reference]));
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use crate::fhirrs::condition::convert_record;
    use crate::fhirutils::constants;
    use serde_json::{json, Value};

    fn meta() -> Value {
        json!({"extension": []})
    }

    fn find<'a>(resources: &'a [Value], resource_type: &str) -> &'a Value {
        resources
            .iter()
            .find(|resource| resource["resourceType"] == json!(resource_type))
            .unwrap()
    }

    #[test]
    fn no_code_produces_no_resource() {
        let record = json!({"patientInternalId": "p1", "conditionCategory": "problem-list-item"});
        assert!(convert_record("g1", &record, &meta()).unwrap().is_empty());
    }

    #[test]
    fn condition_carries_code_status_and_dates() {
        let record = json!({
            "resourceInternalId": "c1",
            "patientInternalId": "p1",
            "conditionCode": "E11.9",
            "conditionCodeSystem": "ICD10",
            "conditionCodeText": "Diabetes",
            "conditionClinicalStatus": "active",
            "conditionVerificationStatus": "confirmed",
            "conditionRecordedDateTime": "2021-06-01 10:00:00",
            "conditionOnsetDateTime": "2021-05-01",
            "conditionAbatementDateTime": "2021-07-01",
            "conditionSeverityCode": "24484000",
            "conditionSeverityText": "Severe",
            "conditionChronicity": "chronic",
            "timeZone": "UTC"
        });

        let resources = convert_record("g1", &record, &meta()).unwrap();
        assert_eq!(resources.len(), 1);
        let condition = &resources[0];

        assert_eq!(condition["resourceType"], json!("Condition"));
        assert_eq!(condition["id"], json!("c1"));
        assert_eq!(condition["subject"], json!({"reference": "Patient/p1"}));
        assert_eq!(
            condition["code"]["coding"][0]["system"],
            json!(constants::ICD10_SYSTEM)
        );
        assert_eq!(
            condition["clinicalStatus"]["coding"][0]["display"],
            json!("Active")
        );
        assert_eq!(
            condition["verificationStatus"]["coding"][0]["display"],
            json!("Confirmed")
        );
        assert_eq!(
            condition["recordedDate"],
            json!("2021-06-01T10:00:00+00:00")
        );
        assert_eq!(
            condition["onsetDateTime"],
            json!("2021-05-01T00:00:00+00:00")
        );
        assert_eq!(
            condition["abatementDateTime"],
            json!("2021-07-01T00:00:00+00:00")
        );
        assert_eq!(condition["severity"]["text"], json!("Severe"));

        let chronicity = &condition["extension"][0];
        assert_eq!(chronicity["url"], json!(constants::EXT_CHRONICITY));
        assert_eq!(chronicity["valueCodeableConcept"]["text"], json!("chronic"));
        assert_eq!(
            chronicity["valueCodeableConcept"]["coding"][0]["code"],
            json!("90734009")
        );

        let ext_id = condition["identifier"]
            .as_array()
            .unwrap()
            .iter()
            .find(|identifier| identifier["id"] == json!("extID"))
            .unwrap();
        assert_eq!(ext_id["value"], json!("E11.9-ICD10"));
    }

    #[test]
    fn encounter_diagnosis_links_the_condition() {
        let record = json!({
            "resourceInternalId": "c1",
            "encounterInternalId": "e1",
            "conditionCode": "E11.9",
            "conditionCodeText": "Diabetes",
            "conditionCategory": "encounter-diagnosis",
            "conditionDiagnosisRank": "2",
            "conditionDiagnosisUse": "AD"
        });

        let resources = convert_record("g1", &record, &meta()).unwrap();
        let encounter = find(&resources, "Encounter");
        let diagnosis = &encounter["diagnosis"][0];

        assert_eq!(diagnosis["id"], json!("2"));
        assert_eq!(diagnosis["rank"], json!(2));
        assert_eq!(diagnosis["use"]["coding"][0]["code"], json!("AD"));
        assert_eq!(
            diagnosis["use"]["coding"][0]["display"],
            json!("Admission diagnosis")
        );
        assert_eq!(
            diagnosis["condition"],
            json!({"reference": "Condition/c1", "display": "E11.9 (Diabetes)"})
        );

        let condition = find(&resources, "Condition");
        assert_eq!(condition["encounter"], json!({"reference": "Encounter/e1"}));
        assert_eq!(
            condition["category"][0]["coding"][0]["code"],
            json!("encounter-diagnosis")
        );
    }

    #[test]
    fn problem_list_items_become_encounter_reason_references() {
        let record = json!({
            "resourceInternalId": "c1",
            "encounterInternalId": "e1",
            "conditionCode": "E11.9",
            "conditionCategory": "problem-list-item"
        });
        let resources = convert_record("g1", &record, &meta()).unwrap();
        let encounter = find(&resources, "Encounter");
        assert_eq!(
            encounter["reasonReference"][0]["reference"],
            json!("Condition/c1")
        );
        assert_eq!(encounter["reasonReference"][0]["id"], json!("Condition-c1"));
    }

    #[test]
    fn diagnosis_rank_alone_implies_the_encounter_diagnosis_category() {
        let record = json!({
            "resourceInternalId": "c1",
            "conditionCode": "E11.9",
            "conditionDiagnosisRank": "1"
        });
        let resources = convert_record("g1", &record, &meta()).unwrap();
        let condition = find(&resources, "Condition");
        assert_eq!(
            condition["category"][0]["coding"][0]["code"],
            json!("encounter-diagnosis")
        );
        assert!(resources
            .iter()
            .any(|resource| resource["resourceType"] == json!("Encounter")));
    }
}
