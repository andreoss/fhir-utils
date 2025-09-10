use crate::error::Error;
use crate::fhirrs::common;
use crate::fhirutils::builders;
use crate::fhirutils::constants;
use crate::fhirutils::identifiers;
use serde_json::{json, Value};

const DEFAULT_CLINICAL_STATUS: &str = "active";
const ENTERED_IN_ERROR: &str = "entered-in-error";

pub fn convert_record(
    group_by_key: &str,
    record: &Value,
    meta: &Value,
) -> Result<Vec<Value>, Error> {
    let record = common::normalized(record);
    let code = builders::field(&record, "allergyCode");
    let code_text = builders::field(&record, "allergyCodeText");
    if code.is_none() && code_text.is_none() {
        return Ok(Vec::new());
    }

    let mut allergy = common::resource(
        "AllergyIntolerance",
        &builders::resource_id(builders::field(&record, "resourceInternalId")),
        meta,
    );

    let code_concept = builders::codeable_concept(
        Some(builders::field(&record, "allergyCodeSystem").unwrap_or(constants::SNOMED_SYSTEM)),
        code,
        None,
        code_text,
    );

    let mut identifier_values = identifiers::identifier_list(&record).unwrap_or_default();
    if let Some(concept) = &code_concept {
        if let Some(external) =
            identifiers::external_identifier(concept, "AllergyIntolerance", None)
        {
            identifier_values.push(external);
        }
    }
    common::insert_list(&mut allergy, "identifier", identifier_values);

    common::insert_str(
        &mut allergy,
        "type",
        builders::field(&record, "allergyType"),
    );
    common::insert_str(
        &mut allergy,
        "criticality",
        builders::field(&record, "allergyCriticality"),
    );
    if let Some(category) = builders::field(&record, "allergyCategory") {
        common::insert_list(&mut allergy, "category", vec![json!(category)]);
    }
    common::insert(
        &mut allergy,
        "patient",
        common::subject_reference(&record, group_by_key),
    );
    common::insert(
        &mut allergy,
        "encounter",
        common::encounter_reference(&record),
    );
    common::insert(&mut allergy, "code", code_concept);

    if let Some(recorded) = builders::field(&record, "allergyRecordedDateTime")
        .and_then(|value| builders::datetime(value, common::zone(&record)))
    {
        allergy.insert("recordedDate".into(), json!(recorded));
    }

    let verification_code = builders::field(&record, "allergyVerificationStatusCode");
    if let Some(code) = verification_code {
        common::insert(
            &mut allergy,
            "verificationStatus",
            builders::codeable_concept(
                Some(constants::ALLERGY_VERIFICATION_STATUS_SYSTEM),
                Some(code),
                constants::display(constants::ALLERGY_VERIFICATION_STATUS_DISPLAY, code),
                None,
            ),
        );
    }

    let clinical_code = builders::field(&record, "allergyClinicalStatusCode").or(
        if verification_code != Some(ENTERED_IN_ERROR) {
            Some(DEFAULT_CLINICAL_STATUS)
        } else {
            None
        },
    );
    if let Some(code) = clinical_code {
        common::insert(
            &mut allergy,
            "clinicalStatus",
            builders::codeable_concept(
                Some(constants::ALLERGY_CLINICAL_STATUS_SYSTEM),
                Some(code),
                constants::display(constants::ALLERGY_CLINICAL_STATUS_DISPLAY, code),
                None,
            ),
        );
    }

    common::insert(
        &mut allergy,
        "onsetPeriod",
        builders::period(
            builders::field(&record, "allergyOnsetStartDateTime"),
            builders::field(&record, "allergyOnsetEndDateTime"),
            common::zone(&record),
        ),
    );

    let manifestations = manifestation_list(&record);
    if !manifestations.is_empty() {
        common::insert_list(
            &mut allergy,
            "reaction",
            vec![json!({"manifestation": manifestations})],
        );
    }

    Ok(vec![Value::Object(allergy)])
}

fn manifestation_list(record: &Value) -> Vec<Value> {
    let code = builders::field(record, "allergyManifestationCode");
    let text = builders::field(record, "allergyManifestationText");
    let list = builders::field_list(record, "allergyManifestationCodeList");

    if code.is_none() && text.is_none() && list.is_empty() {
        return Vec::new();
    }

    let mut manifestations = Vec::new();
    let single = builders::codeable_concept(
        Some(
            builders::field(record, "allergyManifestationSystem")
                .unwrap_or(constants::SNOMED_SYSTEM),
        ),
        code,
        None,
        text,
    );
    if let Some(single) = single {
        manifestations.push(single);
    }
    for entry in list {
        if let Some(concept) = builders::hl7_codeable_concept(&entry) {
            manifestations.push(concept);
        }
    }

    if manifestations.is_empty() {
        manifestations.push(json!({
            "coding": [{
                "system": constants::DATA_ABSENT_SYSTEM,
                "code": "unknown",
                "display": "Unknown"
            }]
        }));
    }
    manifestations
}

#[cfg(test)]
mod tests {
    use crate::fhirrs::allergy_intolerance::convert_record;
    use crate::fhirrs::testing::meta;
    use crate::fhirutils::constants;
    use serde_json::json;

    #[test]
    fn no_code_produces_no_resource() {
        let record = json!({"patientInternalId": "p1", "allergyCriticality": "high"});
        assert!(convert_record("g1", &record, &meta()).unwrap().is_empty());
    }

    #[test]
    fn allergy_defaults_the_clinical_status_and_code_system() {
        let record = json!({
            "patientInternalId": "p1",
            "encounterInternalId": "e1",
            "resourceInternalId": "a1",
            "allergyCode": "227493005",
            "allergyCodeText": "Cashew nuts",
            "allergyCategory": "food",
            "allergyType": "allergy",
            "allergyCriticality": "high",
            "allergyRecordedDateTime": "2021-06-01 10:00:00",
            "timeZone": "UTC"
        });

        let allergy = convert_record("g1", &record, &meta()).unwrap().remove(0);
        assert_eq!(allergy["resourceType"], json!("AllergyIntolerance"));
        assert_eq!(allergy["id"], json!("a1"));
        assert_eq!(allergy["patient"], json!({"reference": "Patient/p1"}));
        assert_eq!(allergy["encounter"], json!({"reference": "Encounter/e1"}));
        assert_eq!(allergy["category"], json!(["food"]));
        assert_eq!(allergy["type"], json!("allergy"));
        assert_eq!(allergy["criticality"], json!("high"));
        assert_eq!(
            allergy["code"]["coding"][0]["system"],
            json!(constants::SNOMED_SYSTEM)
        );
        assert_eq!(allergy["code"]["text"], json!("Cashew nuts"));
        assert_eq!(allergy["recordedDate"], json!("2021-06-01T10:00:00+00:00"));
        assert_eq!(
            allergy["clinicalStatus"]["coding"][0]["code"],
            json!("active")
        );
        assert_eq!(
            allergy["clinicalStatus"]["coding"][0]["display"],
            json!("Active")
        );

        let ext_id = allergy["identifier"]
            .as_array()
            .unwrap()
            .iter()
            .find(|identifier| identifier["id"] == json!("extID"))
            .unwrap();
        assert_eq!(ext_id["value"], json!("227493005-SNOMED"));
    }

    #[test]
    fn entered_in_error_skips_the_default_clinical_status() {
        let record = json!({
            "allergyCode": "1",
            "allergyVerificationStatusCode": "entered-in-error"
        });
        let allergy = convert_record("g1", &record, &meta()).unwrap().remove(0);
        assert!(allergy.get("clinicalStatus").is_none());
        assert_eq!(
            allergy["verificationStatus"]["coding"][0]["display"],
            json!("Entered in Error")
        );
    }

    #[test]
    fn reactions_collect_single_and_list_manifestations() {
        let record = json!({
            "allergyCode": "1",
            "allergyManifestationCode": "247472004",
            "allergyManifestationText": "Hives",
            "allergyManifestationCodeList": ["271807003^Rash^SNOMED"]
        });
        let allergy = convert_record("g1", &record, &meta()).unwrap().remove(0);
        let manifestations = allergy["reaction"][0]["manifestation"].as_array().unwrap();
        assert_eq!(manifestations.len(), 2);
        assert_eq!(manifestations[0]["text"], json!("Hives"));
        assert_eq!(manifestations[1]["coding"][0]["display"], json!("Rash"));
    }

    #[test]
    fn onset_period_uses_the_row_timezone() {
        let record = json!({
            "allergyCode": "1",
            "allergyOnsetStartDateTime": "2021-06-01 08:00:00",
            "timeZone": "America/New_York"
        });
        let allergy = convert_record("g1", &record, &meta()).unwrap().remove(0);
        assert_eq!(
            allergy["onsetPeriod"]["start"],
            json!("2021-06-01T08:00:00-04:00")
        );
        assert!(allergy["reaction"].is_null());
    }
}
