use crate::error::Error;
use crate::fhirrs::common;
use crate::fhirrs::location;
use crate::fhirrs::patient;
use crate::fhirrs::practitioner;
use crate::fhirutils::builders;
use crate::fhirutils::constants;
use crate::fhirutils::identifiers;
use serde_json::{json, Map, Value};

const IDENTIFIER_FIELDS: &[(&str, &str)] = &[
    ("assigningAuthority", "assigningAuthority"),
    ("patientInternalId", "patientInternalId"),
    ("accountNumber", "accountNumber"),
    ("ssn", "ssn"),
    ("ssnSystem", "ssnSystem"),
    ("mrn", "mrn"),
    ("encounterNumber", "encounterNumber"),
    ("encounterInternalId", "resourceInternalId"),
    ("resourceInternalId", "resourceInternalId"),
];

pub fn convert_record(
    group_by_key: &str,
    record: &Value,
    meta: &Value,
) -> Result<Vec<Value>, Error> {
    let record = common::normalized(record);
    let mut resources = Vec::new();
    let encounter = build(group_by_key, &record, meta, &mut resources);
    resources.push(encounter);
    Ok(resources)
}

pub fn build(
    group_by_key: &str,
    record: &Value,
    meta: &Value,
    resources: &mut Vec<Value>,
) -> Value {
    let identifier_record = common::project(record, IDENTIFIER_FIELDS);
    let id = builders::resource_id(builders::field(&identifier_record, "resourceInternalId"));
    let mut encounter = common::resource("Encounter", &id, meta);

    if let Some(identifier_values) = identifiers::identifier_list(&identifier_record) {
        common::insert_list(&mut encounter, "identifier", identifier_values);
    }
    encounter.insert(
        "status".into(),
        json!(builders::field(record, "encounterStatus").unwrap_or("unknown")),
    );
    common::insert(
        &mut encounter,
        "subject",
        common::subject_reference(record, group_by_key),
    );
    common::insert(&mut encounter, "class", encounter_class(record));
    common::insert(
        &mut encounter,
        "period",
        builders::period(
            builders::field(record, "encounterStartDateTime"),
            builders::field(record, "encounterEndDateTime"),
            common::zone(record),
        ),
    );
    common::insert(&mut encounter, "hospitalization", hospitalization(record));
    common::insert(
        &mut encounter,
        "priority",
        builders::codeable_concept(
            builders::field(record, "encounterPriorityCodeSystem")
                .or_else(|| builders::field(record, "encounterPrioritySystem")),
            builders::field(record, "encounterPriorityCode"),
            None,
            builders::field(record, "encounterPriorityText"),
        ),
    );
    common::insert_list(&mut encounter, "statusHistory", status_history(record));
    common::insert(
        &mut encounter,
        "length",
        builders::duration(
            builders::field(record, "encounterLengthValue"),
            builders::field(record, "encounterLengthUnits"),
        ),
    );

    let reason = builders::codeable_concept(
        builders::field(record, "encounterReasonCodeSystem")
            .or_else(|| builders::field(record, "encounterReasonSystem")),
        builders::field(record, "encounterReasonCode"),
        None,
        builders::field(record, "encounterReasonCodeText")
            .or_else(|| builders::field(record, "encounterReasonText")),
    );
    if let Some(reason) = reason {
        common::insert_list(&mut encounter, "reasonCode", vec![reason]);
    }

    resources.extend(patient::convert_record(group_by_key, record, meta).unwrap_or_default());

    let practitioners =
        practitioner::convert_record(group_by_key, record, meta).unwrap_or_default();
    if let Some(participant) = participant(record, practitioners.first()) {
        common::insert_list(&mut encounter, "participant", vec![participant]);
    }
    resources.extend(practitioners);

    if let Some(primary_care) = primary_care_patient(group_by_key, record, meta) {
        resources.push(primary_care);
    }

    let locations = location::convert_record(group_by_key, record, meta).unwrap_or_default();
    common::insert_list(
        &mut encounter,
        "location",
        encounter_locations(record, &locations),
    );
    resources.extend(locations);

    for extension in extensions(record) {
        common::push_extension(&mut encounter, Some(extension));
    }

    Value::Object(encounter)
}

fn encounter_class(record: &Value) -> Option<Value> {
    match builders::field(record, "encounterClassCode") {
        Some(code) => {
            let display = builders::field(record, "encounterClassText")
                .or_else(|| constants::display(constants::ENCOUNTER_CLASS_DISPLAY, code));
            builders::coding(
                Some(code),
                Some(
                    builders::field(record, "encounterClassSystem")
                        .unwrap_or(constants::ENCOUNTER_CLASS_SYSTEM),
                ),
                display,
            )
        }
        None => builders::coding(
            Some("temp-unknown"),
            Some(constants::DATA_ABSENT_SYSTEM),
            Some("Temporarily Unknown"),
        ),
    }
}

fn hospitalization(record: &Value) -> Option<Value> {
    let admit_code = builders::field(record, "hospitalizationAdmitSourceCode");
    let admit_text = builders::field(record, "hospitalizationAdmitSourceCodeText")
        .or_else(|| builders::field(record, "hospitalizationAdmitSourceText"));
    let readmission_code = builders::field(record, "hospitalizationReAdmissionCode");
    let readmission_text = builders::field(record, "hospitalizationReAdmissionCodeText")
        .or_else(|| builders::field(record, "hospitalizationReAdmissionText"));
    let discharge_code = builders::field(record, "hospitalizationDischargeDispositionCode");
    let discharge_text = builders::field(record, "hospitalizationDischargeDispositionCodeText")
        .or_else(|| builders::field(record, "hospitalizationDischargeDispositionText"));

    let present = [
        admit_code,
        admit_text,
        readmission_code,
        readmission_text,
        discharge_code,
        discharge_text,
    ]
    .iter()
    .any(|field| field.is_some());
    if !present {
        return None;
    }

    let mut hospitalization = Map::new();
    common::insert(
        &mut hospitalization,
        "admitSource",
        builders::codeable_concept(
            builders::field(record, "hospitalizationAdmitSourceCodeSystem")
                .or_else(|| builders::field(record, "hospitalizationAdmitSourceSystem"))
                .or(Some(constants::ADMIT_SOURCE_SYSTEM)),
            admit_code,
            admit_code.and_then(|code| constants::display(constants::ADMIT_SOURCE_DISPLAY, code)),
            admit_text,
        ),
    );
    common::insert(
        &mut hospitalization,
        "reAdmission",
        builders::codeable_concept(
            builders::field(record, "hospitalizationReAdmissionCodeSystem")
                .or_else(|| builders::field(record, "hospitalizationReAdmissionSystem"))
                .or(Some(constants::RE_ADMISSION_SYSTEM)),
            readmission_code.map(|_| "R"),
            readmission_code.map(|_| "Re-admission"),
            readmission_text,
        ),
    );
    common::insert(
        &mut hospitalization,
        "dischargeDisposition",
        builders::codeable_concept(
            builders::field(record, "hospitalizationDischargeDispositionCodeSystem")
                .or_else(|| builders::field(record, "hospitalizationDischargeDispositionSystem"))
                .or(Some(constants::DISCHARGE_DISPOSITION_SYSTEM)),
            discharge_code,
            None,
            discharge_text,
        ),
    );

    if hospitalization.is_empty() {
        None
    } else {
        Some(Value::Object(hospitalization))
    }
}

fn status_history(record: &Value) -> Vec<Value> {
    let mut history = Vec::new();
    for entry in builders::field_list(record, "encounterStatusHistory") {
        let parts: Vec<&str> = entry.split('^').collect();
        let status = parts.first().copied().filter(|value| !value.is_empty());
        let start = parts
            .get(1)
            .copied()
            .filter(|value| !value.is_empty() && *value != "None");
        let end = parts
            .get(2)
            .copied()
            .filter(|value| !value.is_empty() && *value != "None");
        if start.is_none() && end.is_none() {
            continue;
        }
        let mut item = Map::new();
        common::insert_str(&mut item, "status", status);
        common::insert(
            &mut item,
            "period",
            builders::period(start, end, common::zone(record)),
        );
        history.push(Value::Object(item));
    }
    history
}

fn participant(record: &Value, practitioner: Option<&Value>) -> Option<Value> {
    let practitioner = practitioner?;
    let mut participant = Map::new();
    common::insert(
        &mut participant,
        "individual",
        common::reference_to(practitioner),
    );

    let mut id = builders::field(record, "encounterParticipantSequenceId").map(String::from);
    if id.is_none() {
        if let Some(npi) = builders::field(record, "practitionerNPI") {
            id = Some(format!("NPI.{npi}"));
        } else if let Some(internal) = builders::field(record, "practitionerInternalId") {
            id = Some(format!("RI.{internal}"));
        }
        let prefix = builders::field(record, "encounterParticipantTypeCode")
            .or_else(|| builders::field(record, "encounterParticipantTypeText"));
        if let (Some(prefix), Some(value)) = (prefix, id.as_deref()) {
            id = Some(format!("{prefix}.{value}"));
        }
    }
    if let Some(id) = id.as_deref().and_then(builders::format_id) {
        participant.insert("id".into(), json!(id));
    }

    let type_code = builders::field(record, "encounterParticipantTypeCode");
    let participant_type = builders::codeable_concept(
        builders::field(record, "encounterParticipantTypeCodeSystem")
            .or_else(|| builders::field(record, "encounterParticipantTypeSystem"))
            .or(Some(constants::PARTICIPANT_TYPE_SYSTEM)),
        type_code,
        type_code.and_then(|code| constants::display(constants::PARTICIPANT_TYPE_DISPLAY, code)),
        builders::field(record, "encounterParticipantTypeText"),
    );
    if let Some(participant_type) = participant_type {
        common::insert_list(&mut participant, "type", vec![participant_type]);
    }

    Some(Value::Object(participant))
}

fn primary_care_patient(group_by_key: &str, record: &Value, meta: &Value) -> Option<Value> {
    if builders::field(record, "encounterParticipantTypeText") != Some("PRIMARY_CARE") {
        return None;
    }
    let role_id = builders::field(record, "practitionerInternalId")?;
    let id = builders::field(record, "patientInternalId").unwrap_or(group_by_key);
    let mut patient = common::resource("Patient", &builders::resource_id(Some(id)), meta);

    let identifier_source = match builders::field(record, "patientInternalId") {
        Some(patient_id) => json!({"patientInternalId": patient_id}),
        None => json!({"accountNumber": builders::field(record, "accountNumber")}),
    };
    if let Some(identifier_values) = identifiers::identifier_list(&identifier_source) {
        common::insert_list(&mut patient, "identifier", identifier_values);
    }
    if let Some(reference) = builders::reference("PractitionerRole", role_id, None) {
        common::insert_list(&mut patient, "generalPractitioner", vec![reference]);
    }
    Some(Value::Object(patient))
}

fn encounter_locations(record: &Value, locations: &[Value]) -> Vec<Value> {
    let mut entries = Vec::new();
    for (index, location) in locations.iter().enumerate() {
        let mut entry = Map::new();
        common::insert(&mut entry, "location", common::reference_to(location));
        if index == 0 && locations.len() == 1 {
            if let Some(id) =
                builders::field(record, "encounterLocationSequenceId").and_then(builders::format_id)
            {
                entry.insert("id".into(), json!(id));
            }
            common::insert(
                &mut entry,
                "period",
                builders::period(
                    builders::field(record, "encounterLocationPeriodStart"),
                    builders::field(record, "encounterLocationPeriodEnd"),
                    common::zone(record),
                ),
            );
        }
        entries.push(Value::Object(entry));
    }
    entries
}

fn extensions(record: &Value) -> Vec<Value> {
    let mut extensions = Vec::new();
    if let Some(insured) = insured_extension(record) {
        extensions.push(insured);
    }
    if let Some(drg) = builders::field(record, "encounterDrgCode") {
        if let Some(extension) =
            builders::extension(constants::DRG_CODE_SYSTEM, "valueString", json!(drg))
        {
            extensions.push(extension);
        }
    }
    if let Some(claim_type) = builders::field(record, "encounterClaimType") {
        if let Some(extension) =
            builders::extension(constants::EXT_CLAIM_TYPE, "valueString", json!(claim_type))
        {
            extensions.push(extension);
        }
    }
    extensions
}

fn insured_extension(record: &Value) -> Option<Value> {
    let category_code = builders::field(record, "encounterInsuredCategoryCode");
    let category_text = builders::field(record, "encounterInsuredCategoryText");
    if category_code.is_none() && category_text.is_none() {
        return None;
    }

    let category = builders::extension_codeable_concept(
        constants::EXT_INSURED_CATEGORY,
        category_code,
        builders::field(record, "encounterInsuredCategorySystem"),
        None,
        category_text,
    )?;

    let mut children = vec![category];
    if let Some(rank) = builders::field(record, "encounterInsuredRank") {
        if let Some(rank) = rank.parse::<i64>().ok().and_then(|rank| {
            builders::extension(constants::EXT_INSURED_RANK, "valueInteger", json!(rank))
        }) {
            children.push(rank);
        }
    }

    let entry_id = builders::field(record, "encounterInsuredEntryId")
        .or_else(|| builders::field(record, "encounterInsuredRank"))
        .or(category_code)
        .or(category_text);

    let mut insured = Map::new();
    insured.insert("url".into(), json!(constants::EXT_INSURED));
    if let Some(entry_id) = entry_id.and_then(builders::format_id) {
        insured.insert("id".into(), json!(entry_id));
    }
    insured.insert("extension".into(), Value::Array(children));
    Some(Value::Object(insured))
}

#[cfg(test)]
mod tests {
    use crate::fhirrs::encounter::convert_record;
    use crate::fhirrs::testing::meta;
    use crate::fhirutils::constants;
    use serde_json::{json, Value};

    fn encounter_of(resources: &[Value]) -> &Value {
        resources
            .iter()
            .find(|resource| resource["resourceType"] == json!("Encounter"))
            .unwrap()
    }

    #[test]
    fn encounter_carries_class_period_and_identifiers() {
        let record = json!({
            "encounterInternalId": "e1",
            "encounterNumber": "v1",
            "patientInternalId": "p1",
            "assigningAuthority": "authority",
            "encounterStatus": "finished",
            "encounterClassCode": "IMP",
            "encounterStartDateTime": "2021-06-01 08:00:00",
            "encounterEndDateTime": "2021-06-03 08:00:00",
            "encounterLengthValue": "2",
            "encounterLengthUnits": "days",
            "timeZone": "UTC"
        });
        let resources = convert_record("g1", &record, &meta()).unwrap();
        let encounter = encounter_of(&resources);

        assert_eq!(encounter["id"], json!("e1"));
        assert_eq!(encounter["status"], json!("finished"));
        assert_eq!(encounter["subject"], json!({"reference": "Patient/p1"}));
        assert_eq!(encounter["class"]["code"], json!("IMP"));
        assert_eq!(
            encounter["class"]["system"],
            json!(constants::ENCOUNTER_CLASS_SYSTEM)
        );
        assert_eq!(encounter["class"]["display"], json!("inpatient encounter"));
        assert_eq!(
            encounter["period"]["start"],
            json!("2021-06-01T08:00:00+00:00")
        );
        assert_eq!(encounter["length"], json!({"value": 2.0, "unit": "days"}));

        let types: Vec<&str> = encounter["identifier"]
            .as_array()
            .unwrap()
            .iter()
            .map(|identifier| identifier["type"]["coding"][0]["code"].as_str().unwrap())
            .collect();
        assert!(types.contains(&"VN"));
        assert!(types.contains(&"RI"));
        assert!(types.contains(&"PI"));
    }

    #[test]
    fn status_defaults_when_the_source_omits_it() {
        let resources =
            convert_record("g1", &json!({"encounterInternalId": "e1"}), &meta()).unwrap();
        assert_eq!(encounter_of(&resources)["status"], json!("unknown"));
    }

    #[test]
    fn missing_class_uses_the_data_absent_code() {
        let resources =
            convert_record("g1", &json!({"encounterInternalId": "e1"}), &meta()).unwrap();
        let encounter = encounter_of(&resources);
        assert_eq!(encounter["class"]["code"], json!("temp-unknown"));
        assert_eq!(
            encounter["class"]["system"],
            json!(constants::DATA_ABSENT_SYSTEM)
        );
    }

    #[test]
    fn hospitalization_and_status_history_are_built() {
        let record = json!({
            "encounterInternalId": "e1",
            "hospitalizationAdmitSourceCode": "emd",
            "hospitalizationReAdmissionCode": "1",
            "hospitalizationDischargeDispositionCode": "home",
            "encounterStatusHistory": ["planned^2021-06-01^2021-06-02", "arrived^None^None"],
            "timeZone": "UTC"
        });
        let resources = convert_record("g1", &record, &meta()).unwrap();
        let encounter = encounter_of(&resources);

        let hospitalization = &encounter["hospitalization"];
        assert_eq!(
            hospitalization["admitSource"]["coding"][0]["display"],
            json!("From accident/emergency department")
        );
        assert_eq!(
            hospitalization["reAdmission"]["coding"][0]["code"],
            json!("R")
        );
        assert_eq!(
            hospitalization["dischargeDisposition"]["coding"][0]["code"],
            json!("home")
        );

        let history = encounter["statusHistory"].as_array().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0]["status"], json!("planned"));
        assert_eq!(
            history[0]["period"]["end"],
            json!("2021-06-02T00:00:00+00:00")
        );
    }

    #[test]
    fn participant_location_and_patient_resources_are_emitted() {
        let record = json!({
            "encounterInternalId": "e1",
            "patientInternalId": "p1",
            "nameLast": "Smith",
            "practitionerInternalId": "pr1",
            "practitionerNameLast": "House",
            "encounterParticipantTypeCode": "ATND",
            "locationResourceInternalId": "l1",
            "locationName": "ICU",
            "encounterLocationSequenceId": "1",
            "encounterLocationPeriodStart": "2021-06-01",
            "timeZone": "UTC"
        });
        let resources = convert_record("g1", &record, &meta()).unwrap();
        let types: Vec<&str> = resources
            .iter()
            .map(|resource| resource["resourceType"].as_str().unwrap())
            .collect();
        assert!(types.contains(&"Patient"));
        assert!(types.contains(&"Practitioner"));
        assert!(types.contains(&"Location"));

        let encounter = encounter_of(&resources);
        assert_eq!(
            encounter["participant"][0]["individual"],
            json!({"reference": "Practitioner/pr1", "display": "House"})
        );
        assert_eq!(encounter["participant"][0]["id"], json!("ATND.RI.pr1"));
        assert_eq!(
            encounter["participant"][0]["type"][0]["coding"][0]["display"],
            json!("attender")
        );
        assert_eq!(
            encounter["location"][0]["location"],
            json!({"reference": "Location/l1", "display": "ICU"})
        );
        assert_eq!(encounter["location"][0]["id"], json!("1"));
        assert_eq!(
            encounter["location"][0]["period"]["start"],
            json!("2021-06-01T00:00:00+00:00")
        );
    }

    #[test]
    fn primary_care_participants_add_a_general_practitioner_patient() {
        let record = json!({
            "encounterInternalId": "e1",
            "patientInternalId": "p1",
            "practitionerInternalId": "pr1",
            "practitionerRoleText": "Primary",
            "encounterParticipantTypeText": "PRIMARY_CARE"
        });
        let resources = convert_record("g1", &record, &meta()).unwrap();
        let patient = resources
            .iter()
            .find(|resource| resource["resourceType"] == json!("Patient"))
            .unwrap();
        assert_eq!(
            patient["generalPractitioner"][0],
            json!({"reference": "PractitionerRole/pr1"})
        );
    }

    #[test]
    fn insured_claim_and_drg_extensions_are_added() {
        let record = json!({
            "encounterInternalId": "e1",
            "encounterInsuredCategoryCode": "SELF",
            "encounterInsuredCategoryText": "Self pay",
            "encounterInsuredRank": "1",
            "encounterClaimType": "professional",
            "encounterDrgCode": "470"
        });
        let resources = convert_record("g1", &record, &meta()).unwrap();
        let encounter = encounter_of(&resources);
        let extensions = encounter["extension"].as_array().unwrap();
        assert_eq!(extensions.len(), 3);

        let insured = &extensions[0];
        assert_eq!(insured["url"], json!(constants::EXT_INSURED));
        assert_eq!(insured["id"], json!("1"));
        assert_eq!(
            insured["extension"][0]["url"],
            json!(constants::EXT_INSURED_CATEGORY)
        );
        assert_eq!(insured["extension"][1]["valueInteger"], json!(1));
        assert_eq!(extensions[1]["url"], json!(constants::DRG_CODE_SYSTEM));
        assert_eq!(extensions[2]["valueString"], json!("professional"));
    }
}
