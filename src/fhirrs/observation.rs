use crate::error::Error;
use crate::fhirrs::common;
use crate::fhirrs::meta as resource_meta;
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
    let code = builders::field(&record, "observationCode");
    let code_text = builders::field(&record, "observationCodeText");
    if code.is_none() && code_text.is_none() {
        return Ok(Vec::new());
    }

    let mut resources = Vec::new();
    let mut observation = common::resource(
        "Observation",
        &builders::resource_id(builders::field(&record, "resourceInternalId")),
        meta,
    );

    let base_code = match code.filter(|code| code.contains('^')) {
        Some(code) => builders::hl7_codeable_concept(code),
        None => builders::codeable_concept(
            builders::field(&record, "observationCodeSystem"),
            code,
            None,
            code_text,
        ),
    };
    let code_concept = builders::add_hl7_coded_list(
        base_code,
        &builders::field_list(&record, "observationCodeList"),
        builders::field(&record, "assigningAuthority"),
    );

    common::insert_str(
        &mut observation,
        "status",
        builders::field(&record, "observationStatus"),
    );
    common::insert(&mut observation, "code", code_concept.clone());
    common::insert(
        &mut observation,
        "subject",
        common::subject_reference(&record, group_by_key),
    );
    common::insert(
        &mut observation,
        "encounter",
        common::encounter_reference(&record),
    );

    if let Some(category) = builders::field(&record, "observationCategory") {
        if let Some(display) = constants::display(constants::OBSERVATION_CATEGORY_DISPLAY, category)
        {
            if let Some(concept) = builders::codeable_concept(
                Some(constants::OBSERVATION_CATEGORY_SYSTEM),
                Some(category),
                Some(display),
                None,
            ) {
                common::insert_list(&mut observation, "category", vec![concept]);
            }
        }
    }

    let effective = builders::field(&record, "observationDateTime")
        .and_then(|value| builders::datetime(value, common::zone(&record)));
    if let Some(effective) = &effective {
        observation.insert("effectiveDateTime".into(), json!(effective));
    }

    let mut identifier_values = identifiers::identifier_list(&record).unwrap_or_default();
    if let Some(concept) = &code_concept {
        let timestamp = effective
            .clone()
            .or_else(|| resource_meta::process_timestamp(meta).map(|value| value.to_string()));
        if let Some(external) =
            identifiers::external_identifier(concept, "Observation", timestamp.as_deref())
        {
            identifier_values.push(external);
        }
    }
    common::insert_list(&mut observation, "identifier", identifier_values);

    if builders::field(&record, "practitionerNPI").is_some()
        || builders::field(&record, "practitionerInternalId").is_some()
    {
        let practitioners = practitioner::convert_record(group_by_key, &record, meta)?;
        if let Some(performer) = practitioners.first().and_then(common::reference_to) {
            common::insert_list(&mut observation, "performer", vec![performer]);
        }
        resources.extend(practitioners);
    }

    set_value(&mut observation, &record);
    if let Some(range) = reference_range(&record) {
        common::insert_list(&mut observation, "referenceRange", vec![range]);
    }
    if let Some(interpretation) = interpretation(&record) {
        common::insert_list(&mut observation, "interpretation", vec![interpretation]);
    }

    resources.push(Value::Object(observation));
    Ok(resources)
}

fn set_value(observation: &mut Map<String, Value>, record: &Value) {
    let value = match builders::field(record, "observationValue") {
        Some(value) => value,
        None => return,
    };
    let units = builders::field(record, "observationValueUnits");
    let data_type = builders::field(record, "observationValueDataType")
        .map(|data_type| data_type.to_string())
        .unwrap_or_else(|| default_data_type(value, units));

    match data_type.as_str() {
        "valueQuantity" => {
            if let Some(quantity) = builders::quantity(Some(value), units) {
                observation.insert("valueQuantity".into(), quantity);
                return;
            }
        }
        "valueInteger" => {
            if let Ok(parsed) = value.parse::<i64>() {
                observation.insert("valueInteger".into(), json!(parsed));
                return;
            }
        }
        "valueBoolean" => {
            if let Some(parsed) = builders::boolean_value(value) {
                observation.insert("valueBoolean".into(), json!(parsed));
                return;
            }
        }
        "valueCodeableConcept" => {
            if let Some(concept) = builders::hl7_codeable_concept(value) {
                observation.insert("valueCodeableConcept".into(), concept);
                return;
            }
        }
        _ => {}
    }

    let text = match units {
        Some(units) => format!("{value} {units}"),
        None => value.to_string(),
    };
    observation.insert("valueString".into(), json!(text));
}

fn default_data_type(value: &str, units: Option<&str>) -> String {
    if units.is_some() {
        return "valueQuantity".into();
    }
    if value.contains('^') {
        return "valueCodeableConcept".into();
    }
    if value.parse::<u64>().is_ok() {
        return "valueInteger".into();
    }
    if builders::is_decimal(value) {
        return "valueQuantity".into();
    }
    if builders::boolean_value(value).is_some() {
        return "valueBoolean".into();
    }
    "valueString".into()
}

fn reference_range(record: &Value) -> Option<Value> {
    let text = builders::field(record, "observationRefRangeText");
    let range = builders::field(record, "observationRefRange");
    let mut low = builders::field(record, "observationRefRangeLow").map(String::from);
    let mut high = builders::field(record, "observationRefRangeHigh").map(String::from);
    if text.is_none() && range.is_none() && low.is_none() && high.is_none() {
        return None;
    }

    let mut reference_range = Map::new();
    let mut range_text = text.map(String::from);
    if let Some(range) = range {
        if range_text.is_none() {
            range_text = Some(range.to_string());
        }
        let parts: Vec<&str> = range.split('-').collect();
        if parts.len() == 2 {
            if low.is_none() {
                low = Some(parts[0].trim().to_string());
            }
            if high.is_none() {
                high = Some(parts[1].trim().to_string());
            }
        }
    }

    let units = builders::field(record, "observationValueUnits");
    let low_quantity = builders::quantity(low.as_deref(), units);
    let high_quantity = builders::quantity(high.as_deref(), units);
    common::insert(&mut reference_range, "low", low_quantity.clone());
    common::insert(&mut reference_range, "high", high_quantity.clone());

    if text.is_none()
        && ((low_quantity.is_none() && low.is_some())
            || (high_quantity.is_none() && high.is_some()))
    {
        range_text = Some(format!(
            "low: {} high: {}",
            low.clone().unwrap_or_default(),
            high.clone().unwrap_or_default()
        ));
    }
    common::insert_str(&mut reference_range, "text", range_text.as_deref());

    if reference_range.is_empty() {
        None
    } else {
        Some(Value::Object(reference_range))
    }
}

fn interpretation(record: &Value) -> Option<Value> {
    let code = builders::field(record, "observationInterpretationCode");
    let text = builders::field(record, "observationInterpretationCodeText")
        .or_else(|| builders::field(record, "observationInterpretationText"));
    if code.is_none() && text.is_none() {
        return None;
    }

    let system = builders::field(record, "observationInterpretationCodeSystem")
        .or_else(|| builders::field(record, "observationInterpretationSystem"))
        .unwrap_or(constants::OBSERVATION_INTERPRETATION_SYSTEM);
    let display = builders::field(record, "observationInterpretationCodeDisplay")
        .or_else(|| builders::field(record, "observationInterpretationDisplay"))
        .or_else(|| {
            if system == constants::OBSERVATION_INTERPRETATION_SYSTEM {
                code.and_then(|code| {
                    constants::display(constants::OBSERVATION_INTERPRETATION_DISPLAY, code)
                })
            } else {
                None
            }
        });

    builders::codeable_concept(Some(system), code, display, text)
}

#[cfg(test)]
mod tests {
    use crate::fhirrs::observation::convert_record;
    use crate::fhirutils::constants;
    use serde_json::{json, Value};

    fn meta() -> Value {
        json!({"extension": [
            {"url": constants::EXT_META_PROCESS_TIMESTAMP, "valueDateTime": "2020-01-02T03:04:05+00:00"}
        ]})
    }

    #[test]
    fn no_code_produces_no_resource() {
        let record = json!({"patientInternalId": "p1", "observationValue": "5"});
        assert!(convert_record("g1", &record, &meta()).unwrap().is_empty());
    }

    #[test]
    fn observation_carries_code_value_and_timestamped_external_id() {
        let record = json!({
            "resourceInternalId": "o1",
            "patientInternalId": "p1",
            "encounterInternalId": "e1",
            "observationCode": "1234-5",
            "observationCodeSystem": "LOINC",
            "observationCodeText": "Glucose",
            "observationStatus": "final",
            "observationCategory": "laboratory",
            "observationDateTime": "2021-06-01 10:00:00",
            "observationValue": "5.5",
            "observationValueUnits": "mg/dL",
            "timeZone": "UTC"
        });

        let observation = convert_record("g1", &record, &meta()).unwrap().remove(0);
        assert_eq!(observation["resourceType"], json!("Observation"));
        assert_eq!(observation["id"], json!("o1"));
        assert_eq!(observation["status"], json!("final"));
        assert_eq!(observation["subject"], json!({"reference": "Patient/p1"}));
        assert_eq!(
            observation["encounter"],
            json!({"reference": "Encounter/e1"})
        );
        assert_eq!(
            observation["category"][0]["coding"][0]["display"],
            json!("Laboratory")
        );
        assert_eq!(
            observation["effectiveDateTime"],
            json!("2021-06-01T10:00:00+00:00")
        );
        assert_eq!(
            observation["valueQuantity"],
            json!({"value": 5.5, "unit": "mg/dL"})
        );

        let ext_id = observation["identifier"]
            .as_array()
            .unwrap()
            .iter()
            .find(|identifier| identifier["id"] == json!("extID"))
            .unwrap();
        assert_eq!(ext_id["value"], json!("20210601100000-1234-5-LOINC"));
    }

    #[test]
    fn external_id_falls_back_to_the_meta_timestamp() {
        let record = json!({"observationCode": "1234-5", "observationCodeSystem": "LOINC"});
        let observation = convert_record("g1", &record, &meta()).unwrap().remove(0);
        let ext_id = observation["identifier"]
            .as_array()
            .unwrap()
            .iter()
            .find(|identifier| identifier["id"] == json!("extID"))
            .unwrap();
        assert_eq!(ext_id["value"], json!("20200102030405-1234-5-LOINC"));
    }

    #[test]
    fn value_typing_follows_the_declared_data_type() {
        let cases = [
            (
                json!({"observationValue": "5", "observationValueDataType": "valueInteger"}),
                "valueInteger",
                json!(5),
            ),
            (
                json!({"observationValue": "true", "observationValueDataType": "valueBoolean"}),
                "valueBoolean",
                json!(true),
            ),
            (
                json!({"observationValue": "abc"}),
                "valueString",
                json!("abc"),
            ),
            (json!({"observationValue": "7"}), "valueInteger", json!(7)),
            (
                json!({"observationValue": "T"}),
                "valueBoolean",
                json!(true),
            ),
        ];

        for (fields, key, expected) in cases {
            let mut record = json!({"observationCode": "1234-5"});
            for (name, value) in fields.as_object().unwrap() {
                record[name] = value.clone();
            }
            let observation = convert_record("g1", &record, &meta()).unwrap().remove(0);
            assert_eq!(observation[key], expected, "case {key}");
        }
    }

    #[test]
    fn coded_values_and_units_are_handled() {
        let coded = json!({"observationCode": "1234-5", "observationValue": "A^Alpha^SNOMED"});
        let observation = convert_record("g1", &coded, &meta()).unwrap().remove(0);
        assert_eq!(
            observation["valueCodeableConcept"]["coding"][0]["system"],
            json!(constants::SNOMED_SYSTEM)
        );

        let text_with_units = json!({"observationCode": "1234-5", "observationValue": "positive", "observationValueUnits": "titer"});
        let observation = convert_record("g1", &text_with_units, &meta())
            .unwrap()
            .remove(0);
        assert_eq!(observation["valueString"], json!("positive titer"));
    }

    #[test]
    fn reference_range_splits_low_and_high() {
        let record = json!({
            "observationCode": "1234-5",
            "observationRefRange": "3.0-7.0",
            "observationValueUnits": "mg/dL"
        });
        let observation = convert_record("g1", &record, &meta()).unwrap().remove(0);
        let range = &observation["referenceRange"][0];
        assert_eq!(range["low"], json!({"value": 3.0, "unit": "mg/dL"}));
        assert_eq!(range["high"], json!({"value": 7.0, "unit": "mg/dL"}));
        assert_eq!(range["text"], json!("3.0-7.0"));

        let non_numeric =
            json!({"observationCode": "1234-5", "observationRefRangeLow": "negative"});
        let observation = convert_record("g1", &non_numeric, &meta())
            .unwrap()
            .remove(0);
        assert_eq!(
            observation["referenceRange"][0]["text"],
            json!("low: negative high: ")
        );
    }

    #[test]
    fn interpretation_and_performer_are_added() {
        let record = json!({
            "observationCode": "1234-5",
            "observationInterpretationCode": "H",
            "practitionerInternalId": "pr1",
            "practitionerNameLast": "House"
        });
        let resources = convert_record("g1", &record, &meta()).unwrap();
        let observation = resources.last().unwrap();
        assert_eq!(
            observation["interpretation"][0]["coding"][0]["display"],
            json!("High")
        );
        assert_eq!(
            observation["performer"][0]["reference"],
            json!("Practitioner/pr1")
        );
        assert!(resources
            .iter()
            .any(|resource| resource["resourceType"] == json!("Practitioner")));
    }
}
