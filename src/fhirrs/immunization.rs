use crate::error::Error;
use crate::fhirrs::common;
use crate::fhirrs::organization;
use crate::fhirutils::builders;
use crate::fhirutils::constants;
use crate::fhirutils::identifiers;
use serde_json::{json, Value};

pub fn convert_record(
    group_by_key: &str,
    record: &Value,
    meta: &Value,
) -> Result<Vec<Value>, Error> {
    let record = common::normalized(record);
    let code = match builders::field(&record, "immunizationVaccineCode") {
        Some(code) => code,
        None => return Ok(Vec::new()),
    };

    let mut resources = Vec::new();
    let mut immunization = common::resource(
        "Immunization",
        &builders::resource_id(builders::field(&record, "resourceInternalId")),
        meta,
    );

    let vaccine = builders::add_hl7_coded_list(
        builders::codeable_concept(
            builders::field(&record, "immunizationVaccineSystem"),
            Some(code),
            builders::field(&record, "immunizationVaccineDisplay"),
            builders::field(&record, "immunizationVaccineText"),
        ),
        &builders::field_list(&record, "immunizationVaccineCodeList"),
        builders::field(&record, "assigningAuthority"),
    );

    let mut identifier_values = identifiers::identifier_list(&record).unwrap_or_default();
    if let Some(vaccine) = &vaccine {
        if let Some(external) = identifiers::external_identifier(vaccine, "Immunization", None) {
            identifier_values.push(external);
        }
    }
    common::insert_list(&mut immunization, "identifier", identifier_values);

    common::insert_str(
        &mut immunization,
        "status",
        builders::field(&record, "immunizationStatus"),
    );
    common::insert(&mut immunization, "vaccineCode", vaccine);
    common::insert(
        &mut immunization,
        "patient",
        common::subject_reference(&record, group_by_key),
    );
    common::insert(
        &mut immunization,
        "encounter",
        common::encounter_reference(&record),
    );

    if let Some(route) = builders::field(&record, "immunizationRouteCode") {
        common::insert(
            &mut immunization,
            "route",
            builders::codeable_concept(
                builders::field(&record, "immunizationRouteSystem"),
                Some(route),
                None,
                builders::field(&record, "immunizationRouteText"),
            ),
        );
    }
    if let Some(site) = builders::field(&record, "immunizationSiteCode") {
        common::insert(
            &mut immunization,
            "site",
            builders::codeable_concept(
                builders::field(&record, "immunizationSiteSystem"),
                Some(site),
                None,
                builders::field(&record, "immunizationSiteText"),
            ),
        );
    }
    if let Some(reason) = builders::field(&record, "immunizationStatusReasonCode") {
        common::insert(
            &mut immunization,
            "statusReason",
            builders::codeable_concept(
                builders::field(&record, "immunizationStatusReasonSystem"),
                Some(reason),
                constants::display(constants::IMMUNIZATION_STATUS_REASON_DISPLAY, reason),
                builders::field(&record, "immunizationStatusReasonText"),
            ),
        );
    }

    common::insert(
        &mut immunization,
        "doseQuantity",
        builders::quantity(
            builders::field(&record, "immunizationDoseQuantity"),
            builders::field(&record, "immunizationDoseUnit"),
        ),
    );

    match builders::field(&record, "immunizationDate")
        .and_then(|value| builders::datetime(value, common::zone(&record)))
    {
        Some(occurrence) => {
            immunization.insert("occurrenceDateTime".into(), json!(occurrence));
        }
        None => {
            immunization.insert("occurrenceString".into(), json!("unknown"));
        }
    }
    if let Some(expiration) =
        builders::field(&record, "immunizationExpirationDate").and_then(builders::date)
    {
        immunization.insert("expirationDate".into(), json!(expiration));
    }

    if builders::field(&record, "organizationName").is_some() {
        let organizations = organization::convert_record(group_by_key, &record, meta)?;
        if let Some(manufacturer) = organizations.first().and_then(common::reference_to) {
            immunization.insert("manufacturer".into(), manufacturer);
        }
        resources.extend(organizations);
    }

    resources.push(Value::Object(immunization));
    Ok(resources)
}

#[cfg(test)]
mod tests {
    use crate::fhirrs::immunization::convert_record;
    use crate::fhirutils::constants;
    use serde_json::{json, Value};

    fn meta() -> Value {
        json!({"extension": []})
    }

    #[test]
    fn no_vaccine_code_produces_no_resource() {
        let record = json!({"patientInternalId": "p1", "immunizationStatus": "completed"});
        assert!(convert_record("g1", &record, &meta()).unwrap().is_empty());
    }

    #[test]
    fn immunization_carries_vaccine_dose_and_manufacturer() {
        let record = json!({
            "resourceInternalId": "i1",
            "patientInternalId": "p1",
            "encounterInternalId": "e1",
            "immunizationVaccineCode": "208",
            "immunizationVaccineSystem": "CVX",
            "immunizationVaccineText": "COVID-19 vaccine",
            "immunizationStatus": "completed",
            "immunizationRouteCode": "IM",
            "immunizationSiteCode": "LA",
            "immunizationDoseQuantity": "0.3",
            "immunizationDoseUnit": "mL",
            "immunizationDate": "2021-06-01",
            "immunizationExpirationDate": "2021-12-31",
            "organizationResourceInternalId": "o1",
            "organizationName": "Vaccine Co",
            "timeZone": "UTC"
        });

        let resources = convert_record("g1", &record, &meta()).unwrap();
        assert_eq!(resources.len(), 2);
        let immunization = resources.last().unwrap();

        assert_eq!(immunization["resourceType"], json!("Immunization"));
        assert_eq!(immunization["id"], json!("i1"));
        assert_eq!(immunization["status"], json!("completed"));
        assert_eq!(
            immunization["vaccineCode"]["coding"][0]["system"],
            json!(constants::CVX_SYSTEM)
        );
        assert_eq!(immunization["patient"], json!({"reference": "Patient/p1"}));
        assert_eq!(
            immunization["encounter"],
            json!({"reference": "Encounter/e1"})
        );
        assert_eq!(
            immunization["doseQuantity"],
            json!({"value": 0.3, "unit": "mL"})
        );
        assert_eq!(
            immunization["occurrenceDateTime"],
            json!("2021-06-01T00:00:00+00:00")
        );
        assert_eq!(immunization["expirationDate"], json!("2021-12-31"));
        assert_eq!(
            immunization["manufacturer"],
            json!({"reference": "Organization/o1", "display": "Vaccine Co"})
        );

        let ext_id = immunization["identifier"]
            .as_array()
            .unwrap()
            .iter()
            .find(|identifier| identifier["id"] == json!("extID"))
            .unwrap();
        assert_eq!(ext_id["value"], json!("208-CVX"));
    }

    #[test]
    fn missing_date_uses_the_unknown_occurrence_string() {
        let record = json!({"immunizationVaccineCode": "208"});
        let immunization = convert_record("g1", &record, &meta()).unwrap().remove(0);
        assert_eq!(immunization["occurrenceString"], json!("unknown"));
        assert!(immunization.get("occurrenceDateTime").is_none());
    }

    #[test]
    fn vaccine_code_list_adds_codings_with_the_assigning_authority() {
        let record = json!({
            "immunizationVaccineCode": "208",
            "immunizationVaccineSystem": "CVX",
            "immunizationVaccineCodeList": ["J07BX03^Covid vaccine"],
            "assigningAuthority": "authority",
            "immunizationStatusReasonCode": "IMMUNE"
        });
        let immunization = convert_record("g1", &record, &meta()).unwrap().remove(0);
        let codings = immunization["vaccineCode"]["coding"].as_array().unwrap();
        assert_eq!(codings.len(), 2);
        assert_eq!(codings[1]["system"], json!("urn:id:authority"));
        assert_eq!(
            immunization["statusReason"]["coding"][0]["display"],
            json!("immunity")
        );
    }
}
