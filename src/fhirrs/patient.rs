use crate::error::Error;
use crate::fhirrs::common;
use crate::fhirutils::builders;
use crate::fhirutils::constants;
use crate::fhirutils::identifiers;
use serde_json::{json, Value};

const DATA_FIELDS: &[&str] = &[
    "ssn",
    "driversLicense",
    "mrn",
    "nameFirst",
    "nameFirstMiddle",
    "nameMiddle",
    "nameLast",
    "nameFirstMiddleLast",
    "prefix",
    "suffix",
    "birthDate",
    "deceasedDateTime",
    "deceasedBoolean",
    "multipleBirthBoolean",
    "multipleBirthInteger",
    "address1",
    "address2",
    "city",
    "state",
    "postalCode",
    "country",
    "addressText",
    "telecomPhone",
    "race",
    "ethnicity",
    "gender",
];

pub fn convert_record(
    group_by_key: &str,
    record: &Value,
    meta: &Value,
) -> Result<Vec<Value>, Error> {
    let record = common::normalized(record);
    if !contains_patient_data(&record) {
        return Ok(Vec::new());
    }
    Ok(vec![build(group_by_key, &record, meta)])
}

pub fn contains_patient_data(record: &Value) -> bool {
    DATA_FIELDS
        .iter()
        .any(|field| builders::field(record, field).is_some())
}

pub fn build(group_by_key: &str, record: &Value, meta: &Value) -> Value {
    let id = builders::field(record, "patientInternalId").unwrap_or(group_by_key);
    let mut patient = common::resource("Patient", &builders::resource_id(Some(id)), meta);

    if let Some(identifiers) = identifiers::identifier_list(record) {
        common::insert_list(&mut patient, "identifier", identifiers);
    }
    if let Some(name) = common::human_name(record) {
        common::insert_list(&mut patient, "name", vec![name]);
    }
    common::insert_str(&mut patient, "gender", builders::field(record, "gender"));
    if let Some(phone) = builders::field(record, "telecomPhone") {
        common::insert(
            &mut patient,
            "telecom",
            builders::contact_point_phone(phone),
        );
    }

    let address = builders::address(
        builders::field(record, "address1"),
        builders::field(record, "address2"),
        builders::field(record, "city"),
        builders::field(record, "state"),
        builders::field(record, "postalCode"),
        builders::field(record, "country"),
        builders::field(record, "addressText"),
    );
    if let Some(address) = address {
        common::insert_list(&mut patient, "address", vec![address]);
    }

    if let Some(birth_date) = builders::field(record, "birthDate").and_then(builders::date) {
        patient.insert("birthDate".into(), json!(birth_date));
    }

    let deceased = builders::field(record, "deceasedDateTime").and_then(builders::date);
    match deceased {
        Some(deceased) => {
            patient.insert("deceasedDateTime".into(), json!(deceased));
        }
        None => {
            if let Some(value) =
                builders::field(record, "deceasedBoolean").and_then(builders::boolean_value)
            {
                patient.insert("deceasedBoolean".into(), json!(value));
            }
        }
    }

    let multiple_birth =
        builders::field(record, "multipleBirthBoolean").and_then(builders::boolean_value);
    match multiple_birth {
        Some(true) => {
            patient.insert("multipleBirthBoolean".into(), json!(true));
        }
        _ => {
            let count = builders::field(record, "multipleBirthInteger")
                .and_then(|value| value.parse::<i64>().ok());
            match count {
                Some(count) => {
                    patient.insert("multipleBirthInteger".into(), json!(count));
                }
                None => {
                    if multiple_birth == Some(false) {
                        patient.insert("multipleBirthBoolean".into(), json!(false));
                    }
                }
            }
        }
    }

    let race = builders::field(record, "race");
    common::push_extension(
        &mut patient,
        builders::extension_codeable_concept(
            constants::EXT_RACE,
            race,
            Some(builders::field(record, "raceSystem").unwrap_or(constants::RACE_SYSTEM)),
            race.and_then(|code| constants::display(constants::RACE_DISPLAY, code)),
            builders::field(record, "raceText"),
        ),
    );

    let ethnicity = builders::field(record, "ethnicity");
    common::push_extension(
        &mut patient,
        builders::extension_codeable_concept(
            constants::EXT_ETHNICITY,
            ethnicity,
            Some(builders::field(record, "ethnicitySystem").unwrap_or(constants::ETHNICITY_SYSTEM)),
            ethnicity.and_then(|code| constants::display(constants::ETHNICITY_DISPLAY, code)),
            builders::field(record, "ethnicityText"),
        ),
    );

    if let Some(weeks) = builders::field(record, "ageInWeeksForAgeUnder2Years")
        .and_then(|value| value.parse::<u64>().ok())
    {
        common::push_extension(
            &mut patient,
            builders::extension(
                constants::EXT_AGE_IN_WEEKS,
                "valueUnsignedInt",
                json!(weeks),
            ),
        );
    }
    if let Some(months) = builders::field(record, "ageInMonthsForAgeUnder8Years")
        .and_then(|value| value.parse::<u64>().ok())
    {
        common::push_extension(
            &mut patient,
            builders::extension(
                constants::EXT_AGE_IN_MONTHS,
                "valueUnsignedInt",
                json!(months),
            ),
        );
    }

    Value::Object(patient)
}

#[cfg(test)]
mod tests {
    use crate::fhirrs::patient::convert_record;
    use crate::fhirrs::testing::meta;
    use crate::fhirutils::constants;
    use serde_json::json;

    #[test]
    fn no_demographic_data_produces_no_resource() {
        let record = json!({"patientInternalId": "p1", "accountNumber": "a1"});
        assert!(convert_record("g1", &record, &meta()).unwrap().is_empty());
    }

    #[test]
    fn patient_carries_demographics_and_identifiers() {
        let record = json!({
            "patientInternalId": "p1",
            "assigningAuthority": "authority",
            "mrn": "m1",
            "ssn": "123-45-6789",
            "nameLast": "Smith",
            "nameFirst": "Ann",
            "gender": "female",
            "birthDate": "01/02/1980",
            "telecomPhone": "555-1234",
            "address1": "1 Main St",
            "city": "Boston",
            "state": "MA",
            "postalCode": "02101"
        });

        let resources = convert_record("g1", &record, &meta()).unwrap();
        assert_eq!(resources.len(), 1);
        let patient = &resources[0];

        assert_eq!(patient["resourceType"], json!("Patient"));
        assert_eq!(patient["id"], json!("p1"));
        assert_eq!(patient["gender"], json!("female"));
        assert_eq!(patient["birthDate"], json!("1980-01-02"));
        assert_eq!(patient["name"][0]["text"], json!("Ann Smith"));
        assert_eq!(patient["telecom"][0]["value"], json!("555-1234"));
        assert_eq!(patient["address"][0]["city"], json!("Boston"));
        assert!(patient["meta"].is_object());

        let ssn = patient["identifier"]
            .as_array()
            .unwrap()
            .iter()
            .find(|identifier| identifier["type"]["coding"][0]["code"] == json!("SS"))
            .unwrap();
        assert_eq!(ssn["value"], json!("123456789"));
    }

    #[test]
    fn patient_id_falls_back_to_the_group_key() {
        let record = json!({"nameLast": "Smith"});
        let patient = convert_record("g1", &record, &meta()).unwrap().remove(0);
        assert_eq!(patient["id"], json!("g1"));
    }

    #[test]
    fn race_and_ethnicity_use_default_systems_and_displays() {
        let record = json!({"race": "2106-3", "ethnicity": "2135-2"});
        let patient = convert_record("g1", &record, &meta()).unwrap().remove(0);
        let extensions = patient["extension"].as_array().unwrap();

        let race = &extensions[0];
        assert_eq!(race["url"], json!(constants::EXT_RACE));
        assert_eq!(
            race["valueCodeableConcept"]["coding"][0]["system"],
            json!(constants::RACE_SYSTEM)
        );
        assert_eq!(
            race["valueCodeableConcept"]["coding"][0]["display"],
            json!("White")
        );

        let ethnicity = &extensions[1];
        assert_eq!(ethnicity["url"], json!(constants::EXT_ETHNICITY));
        assert_eq!(
            ethnicity["valueCodeableConcept"]["text"],
            json!("Hispanic or Latino")
        );
    }

    #[test]
    fn age_extensions_are_unsigned_integers() {
        let record = json!({
            "nameLast": "Smith",
            "ageInWeeksForAgeUnder2Years": "30",
            "ageInMonthsForAgeUnder8Years": "7"
        });
        let patient = convert_record("g1", &record, &meta()).unwrap().remove(0);
        let extensions = patient["extension"].as_array().unwrap();
        assert_eq!(extensions[0]["url"], json!(constants::EXT_AGE_IN_WEEKS));
        assert_eq!(extensions[0]["valueUnsignedInt"], json!(30));
        assert_eq!(extensions[1]["valueUnsignedInt"], json!(7));
    }

    #[test]
    fn deceased_and_multiple_birth_prefer_specific_values() {
        let deceased = json!({"nameLast": "Smith", "deceasedDateTime": "2020-01-01", "deceasedBoolean": "true"});
        let patient = convert_record("g1", &deceased, &meta()).unwrap().remove(0);
        assert_eq!(patient["deceasedDateTime"], json!("2020-01-01"));
        assert!(patient.get("deceasedBoolean").is_none());

        let flagged = json!({"nameLast": "Smith", "deceasedBoolean": "T"});
        let patient = convert_record("g1", &flagged, &meta()).unwrap().remove(0);
        assert_eq!(patient["deceasedBoolean"], json!(true));

        let twins = json!({"nameLast": "Smith", "multipleBirthInteger": "2"});
        let patient = convert_record("g1", &twins, &meta()).unwrap().remove(0);
        assert_eq!(patient["multipleBirthInteger"], json!(2));

        let single = json!({"nameLast": "Smith", "multipleBirthBoolean": "true"});
        let patient = convert_record("g1", &single, &meta()).unwrap().remove(0);
        assert_eq!(patient["multipleBirthBoolean"], json!(true));
    }
}
