use crate::error::Error;
use crate::fhirrs::common;
use crate::fhirrs::encounter;
use crate::fhirutils::builders;
use crate::fhirutils::constants;
use crate::fhirutils::identifiers;
use serde_json::{json, Map, Value};

pub const ADMINISTRATION: &str = "MedicationAdministration";
pub const REQUEST: &str = "MedicationRequest";
pub const STATEMENT: &str = "MedicationStatement";

const DEFAULT_STATUS: &str = "unknown";

pub fn convert_use(group_by_key: &str, record: &Value, meta: &Value) -> Result<Vec<Value>, Error> {
    let kind = builders::field(record, "resourceType")
        .filter(|kind| [ADMINISTRATION, REQUEST, STATEMENT].contains(kind))
        .unwrap_or(STATEMENT);
    build(kind, group_by_key, record, meta)
}

pub fn convert_administration(
    group_by_key: &str,
    record: &Value,
    meta: &Value,
) -> Result<Vec<Value>, Error> {
    let occurrence = builders::field(record, "medicationUseOccuranceDateTime")
        .and_then(|value| builders::datetime(value, common::zone(record)));
    let kind = if occurrence.is_some() {
        ADMINISTRATION
    } else {
        STATEMENT
    };
    build(kind, group_by_key, record, meta)
}

pub fn convert_request(
    group_by_key: &str,
    record: &Value,
    meta: &Value,
) -> Result<Vec<Value>, Error> {
    build(REQUEST, group_by_key, record, meta)
}

pub fn convert_statement(
    group_by_key: &str,
    record: &Value,
    meta: &Value,
) -> Result<Vec<Value>, Error> {
    build(STATEMENT, group_by_key, record, meta)
}

fn build(
    kind: &str,
    group_by_key: &str,
    record: &Value,
    meta: &Value,
) -> Result<Vec<Value>, Error> {
    let record = common::normalized(record);
    let code = builders::field(&record, "medicationCode");
    let code_text = builders::field(&record, "medicationCodeText");
    let code_list = builders::field_list(&record, "medicationCodeList");
    if code.is_none() && code_text.is_none() && code_list.is_empty() {
        return Ok(Vec::new());
    }

    let medication = builders::add_hl7_coded_list(
        builders::codeable_concept_no_text_default(
            builders::field(&record, "medicationCodeSystem"),
            code,
            builders::field(&record, "medicationCodeDisplay"),
            code_text,
        ),
        &code_list,
        builders::field(&record, "assigningAuthority"),
    );

    let mut resource = common::resource(
        kind,
        &builders::resource_id(builders::field(&record, "resourceInternalId")),
        meta,
    );

    let mut identifier_values = identifiers::identifier_list(&record).unwrap_or_default();
    if let Some(medication) = &medication {
        if let Some(external) = identifiers::external_identifier(medication, kind, None) {
            identifier_values.push(external);
        }
    }
    common::insert_list(&mut resource, "identifier", identifier_values);

    let status = builders::field(&record, "medicationUseStatus").unwrap_or(DEFAULT_STATUS);
    resource.insert("status".into(), json!(status));
    common::insert(&mut resource, "medicationCodeableConcept", medication);
    common::insert(
        &mut resource,
        "subject",
        common::subject_reference(&record, group_by_key),
    );

    let zone = common::zone(&record);
    let encounter_reference = common::encounter_reference(&record);
    let occurrence = builders::field(&record, "medicationUseOccuranceDateTime")
        .and_then(|value| builders::datetime(value, zone));

    let mut resources = Vec::new();
    match kind {
        ADMINISTRATION => {
            common::insert(&mut resource, "context", encounter_reference);
            if let Some(occurrence) = occurrence {
                resource.insert("effectiveDateTime".into(), json!(occurrence));
            }
        }
        REQUEST => {
            common::insert(&mut resource, "encounter", encounter_reference);
            if let Some(authored) = builders::field(&record, "medicationAuthoredOn")
                .and_then(|value| builders::datetime(value, zone))
            {
                resource.insert("authoredOn".into(), json!(authored));
            }
            common::insert_str(
                &mut resource,
                "intent",
                builders::field(&record, "medicationRequestIntent"),
            );
            common::insert(&mut resource, "dispenseRequest", dispense_request(&record));
            if builders::field(&record, "encounterClaimType").is_some()
                || builders::field(&record, "encounterClassCode").is_some()
            {
                let mut linked = Vec::new();
                let encounter = encounter::build(group_by_key, &record, meta, &mut linked);
                resources.push(encounter);
            }
        }
        _ => {
            common::insert(&mut resource, "context", encounter_reference);
            if let Some(occurrence) = occurrence {
                resource.insert("effectiveDateTime".into(), json!(occurrence));
            }
        }
    }

    common::insert(
        &mut resource,
        "category",
        builders::codeable_concept(
            Some(category_system(kind)),
            builders::field(&record, "medicationUseCategoryCode"),
            None,
            builders::field(&record, "medicationUseCategoryCodeText")
                .or_else(|| builders::field(&record, "medicationUseCategoryText")),
        ),
    );

    add_dosage(kind, &mut resource, &record);

    resources.push(Value::Object(resource));
    Ok(resources)
}

fn category_system(kind: &str) -> &'static str {
    match kind {
        ADMINISTRATION => constants::MED_ADM_CATEGORY_SYSTEM,
        REQUEST => constants::MED_REQ_CATEGORY_SYSTEM,
        _ => constants::MED_STM_CATEGORY_SYSTEM,
    }
}

fn dispense_request(record: &Value) -> Option<Value> {
    let mut dispense = Map::new();
    if let Some(refills) =
        builders::field(record, "medicationRefills").and_then(|refills| refills.parse::<i64>().ok())
    {
        dispense.insert("numberOfRepeatsAllowed".into(), json!(refills));
    }
    common::insert(
        &mut dispense,
        "validityPeriod",
        builders::period(
            builders::field(record, "medicationValidityStart"),
            builders::field(record, "medicationValidityEnd"),
            common::zone(record),
        ),
    );
    if let Some(quantity) = builders::field(record, "medicationQuantity")
        .and_then(|quantity| quantity.parse::<i64>().ok())
    {
        dispense.insert("quantity".into(), json!({"value": quantity}));
    }

    if dispense.is_empty() {
        None
    } else {
        Some(Value::Object(dispense))
    }
}

fn add_dosage(kind: &str, resource: &mut Map<String, Value>, record: &Value) {
    let text = builders::field(record, "medicationUseDosageText");
    let route_code = builders::field(record, "medicationUseRouteCode");
    let route_text = builders::field(record, "medicationUseRouteText");
    let value = builders::field(record, "medicationUseDosageValue");
    if text.is_none() && route_code.is_none() && route_text.is_none() && value.is_none() {
        return;
    }

    let route = builders::add_hl7_coded_list(
        builders::codeable_concept(
            builders::field(record, "medicationUseRouteCodeSystem")
                .or_else(|| builders::field(record, "medicationUseRouteSystem")),
            route_code,
            None,
            route_text,
        ),
        &builders::field_list(record, "medicationUseRouteList"),
        builders::field(record, "assigningAuthority"),
    );
    let dose = builders::quantity(value, builders::field(record, "medicationUseDosageUnit"));

    let mut dosage = Map::new();
    common::insert_str(&mut dosage, "text", text);
    common::insert(&mut dosage, "route", route);

    if kind == ADMINISTRATION {
        match dose {
            Some(dose) => {
                dosage.insert("dose".into(), dose);
            }
            None => {
                let absent =
                    builders::extension(constants::EXT_DATA_ABSENT, "valueCode", json!("as-text"));
                if let Some(absent) = absent {
                    dosage.insert("dose".into(), json!({"extension": [absent]}));
                }
            }
        }
        resource.insert("dosage".into(), Value::Object(dosage));
        return;
    }

    if let Some(dose) = dose {
        dosage.insert(
            "doseAndRate".into(),
            Value::Array(vec![json!({"doseQuantity": dose})]),
        );
    }

    let key = if kind == REQUEST {
        "dosageInstruction"
    } else {
        "dosage"
    };
    resource.insert(key.into(), Value::Array(vec![Value::Object(dosage)]));
}

#[cfg(test)]
mod tests {
    use crate::fhirrs::medication::*;
    use crate::fhirutils::constants;
    use serde_json::{json, Value};

    fn meta() -> Value {
        json!({"extension": []})
    }

    #[test]
    fn no_medication_code_produces_no_resource() {
        let record = json!({"patientInternalId": "p1", "medicationUseStatus": "active"});
        assert!(convert_use("g1", &record, &meta()).unwrap().is_empty());
        assert!(convert_request("g1", &record, &meta()).unwrap().is_empty());
    }

    #[test]
    fn medication_use_defaults_to_a_statement() {
        let record = json!({
            "resourceInternalId": "m1",
            "patientInternalId": "p1",
            "encounterInternalId": "e1",
            "medicationCode": "1049502",
            "medicationCodeSystem": "RXNORM",
            "medicationUseOccuranceDateTime": "2021-06-01 10:00:00",
            "medicationUseDosageText": "1 tablet daily",
            "medicationUseDosageValue": "1",
            "medicationUseDosageUnit": "tablet",
            "medicationUseRouteCode": "PO",
            "timeZone": "UTC"
        });

        let statement = convert_use("g1", &record, &meta()).unwrap().remove(0);
        assert_eq!(statement["resourceType"], json!("MedicationStatement"));
        assert_eq!(statement["id"], json!("m1"));
        assert_eq!(statement["status"], json!("unknown"));
        assert_eq!(statement["context"], json!({"reference": "Encounter/e1"}));
        assert_eq!(
            statement["effectiveDateTime"],
            json!("2021-06-01T10:00:00+00:00")
        );
        assert_eq!(
            statement["medicationCodeableConcept"]["coding"][0]["system"],
            json!(constants::RXNORM_SYSTEM)
        );
        assert_eq!(statement["dosage"][0]["text"], json!("1 tablet daily"));
        assert_eq!(
            statement["dosage"][0]["doseAndRate"][0]["doseQuantity"],
            json!({"value": 1.0, "unit": "tablet"})
        );

        let ext_id = statement["identifier"]
            .as_array()
            .unwrap()
            .iter()
            .find(|identifier| identifier["id"] == json!("extID"))
            .unwrap();
        assert_eq!(ext_id["value"], json!("1049502-RXNORM"));
    }

    #[test]
    fn resource_type_field_selects_the_medication_resource() {
        let record = json!({
            "resourceType": "MedicationRequest",
            "medicationCode": "1049502",
            "medicationRequestIntent": "order",
            "medicationRefills": "3",
            "medicationQuantity": "30",
            "medicationValidityStart": "2021-06-01",
            "medicationAuthoredOn": "2021-05-30",
            "medicationUseCategoryCode": "outpatient",
            "medicationUseDosageValue": "2",
            "timeZone": "UTC"
        });

        let request = convert_use("g1", &record, &meta()).unwrap().remove(0);
        assert_eq!(request["resourceType"], json!("MedicationRequest"));
        assert_eq!(request["intent"], json!("order"));
        assert_eq!(request["authoredOn"], json!("2021-05-30T00:00:00+00:00"));
        assert_eq!(
            request["dispenseRequest"]["numberOfRepeatsAllowed"],
            json!(3)
        );
        assert_eq!(request["dispenseRequest"]["quantity"], json!({"value": 30}));
        assert_eq!(
            request["dispenseRequest"]["validityPeriod"]["start"],
            json!("2021-06-01T00:00:00+00:00")
        );
        assert_eq!(
            request["category"]["coding"][0]["system"],
            json!(constants::MED_REQ_CATEGORY_SYSTEM)
        );
        assert_eq!(
            request["dosageInstruction"][0]["doseAndRate"][0]["doseQuantity"]["value"],
            json!(2.0)
        );
    }

    #[test]
    fn administration_needs_an_occurrence_datetime() {
        let with_time = json!({
            "medicationCode": "1049502",
            "medicationUseOccuranceDateTime": "2021-06-01 10:00:00",
            "medicationUseDosageText": "as needed",
            "timeZone": "UTC"
        });
        let administration = convert_administration("g1", &with_time, &meta())
            .unwrap()
            .remove(0);
        assert_eq!(
            administration["resourceType"],
            json!("MedicationAdministration")
        );
        assert_eq!(
            administration["dosage"]["dose"]["extension"][0]["url"],
            json!(constants::EXT_DATA_ABSENT)
        );

        let without_time = json!({"medicationCode": "1049502"});
        let statement = convert_administration("g1", &without_time, &meta())
            .unwrap()
            .remove(0);
        assert_eq!(statement["resourceType"], json!("MedicationStatement"));
    }

    #[test]
    fn requests_with_encounter_data_emit_an_encounter() {
        let record = json!({
            "medicationCode": "1049502",
            "encounterInternalId": "e1",
            "encounterClassCode": "AMB"
        });
        let resources = convert_request("g1", &record, &meta()).unwrap();
        assert_eq!(resources.len(), 2);
        assert_eq!(resources[0]["resourceType"], json!("Encounter"));
        assert_eq!(resources[1]["resourceType"], json!("MedicationRequest"));
    }

    #[test]
    fn statements_use_the_statement_category_system() {
        let record = json!({"medicationCode": "1", "medicationUseCategoryCode": "inpatient"});
        let statement = convert_statement("g1", &record, &meta()).unwrap().remove(0);
        assert_eq!(
            statement["category"]["coding"][0]["system"],
            json!(constants::MED_STM_CATEGORY_SYSTEM)
        );
    }
}
