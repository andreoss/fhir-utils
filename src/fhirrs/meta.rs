use crate::contract::General;
use crate::fhirutils::constants;
use crate::fhirutils::identifiers::now_rfc3339;
use serde_json::{json, Value};

pub fn create_meta(file_name: &str, resource_type: &str, general: &General) -> Value {
    json!({
        "extension": [
            {
                "url": constants::EXT_META_TENANT_ID,
                "valueString": general.tenant_id,
            },
            {
                "url": constants::EXT_META_SOURCE_FILE_ID,
                "valueString": file_name,
            },
            {
                "url": constants::EXT_META_SOURCE_EVENT_TRIGGER,
                "valueCodeableConcept": {"text": resource_type},
            },
            {
                "url": constants::EXT_META_PROCESS_TIMESTAMP,
                "valueDateTime": now_rfc3339(),
            },
            {
                "url": constants::EXT_META_SOURCE_RECORD_TYPE,
                "valueCodeableConcept": {"text": "csv"},
            },
        ]
    })
}

pub fn meta_for_row(meta: &Value, row_num: usize, source_record_id: Option<&str>) -> Value {
    let mut meta = meta.clone();
    let extensions = match meta
        .get_mut("extension")
        .and_then(|value| value.as_array_mut())
    {
        Some(extensions) => extensions,
        None => return meta,
    };

    for extension in extensions.iter_mut() {
        if extension["url"] == json!(constants::EXT_META_SOURCE_FILE_ID) {
            if let Some(current) = extension["valueString"].as_str() {
                let base = current.split(':').next().unwrap_or(current).to_string();
                extension["valueString"] = json!(format!("{base}:{row_num:05}"));
            }
            break;
        }
    }

    if let Some(source_record_id) = source_record_id.filter(|value| !value.is_empty()) {
        extensions.push(json!({
            "url": constants::EXT_META_SOURCE_RECORD_ID,
            "valueString": source_record_id,
        }));
    }

    meta
}

pub fn process_timestamp(meta: &Value) -> Option<&str> {
    meta.get("extension")?
        .as_array()?
        .iter()
        .find(|extension| extension["url"] == json!(constants::EXT_META_PROCESS_TIMESTAMP))
        .and_then(|extension| extension["valueDateTime"].as_str())
}

#[cfg(test)]
mod tests {
    use crate::contract::General;
    use crate::fhirrs::meta::*;
    use crate::fhirutils::constants;
    use serde_json::{json, Value};

    fn general() -> General {
        General {
            time_zone: "America/New_York".into(),
            tenant_id: "tenant1".into(),
            stream_type: "live".into(),
            assigning_authority: None,
            empty_field_values: None,
            regex_filenames: false,
        }
    }

    fn extension<'a>(meta: &'a Value, url: &str) -> &'a Value {
        meta["extension"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["url"] == json!(url))
            .unwrap()
    }

    #[test]
    fn meta_carries_the_source_extensions() {
        let meta = create_meta("patient.csv", "Patient", &general());
        assert_eq!(meta["extension"].as_array().unwrap().len(), 5);
        assert_eq!(
            extension(&meta, constants::EXT_META_TENANT_ID)["valueString"],
            json!("tenant1")
        );
        assert_eq!(
            extension(&meta, constants::EXT_META_SOURCE_FILE_ID)["valueString"],
            json!("patient.csv")
        );
        assert_eq!(
            extension(&meta, constants::EXT_META_SOURCE_EVENT_TRIGGER)["valueCodeableConcept"]
                ["text"],
            json!("Patient")
        );
        assert_eq!(
            extension(&meta, constants::EXT_META_SOURCE_RECORD_TYPE)["valueCodeableConcept"]
                ["text"],
            json!("csv")
        );
        assert!(process_timestamp(&meta).is_some());
    }

    #[test]
    fn row_meta_suffixes_the_source_file_id() {
        let meta = create_meta("patient.csv", "Patient", &general());

        let row = meta_for_row(&meta, 7, None);
        assert_eq!(
            extension(&row, constants::EXT_META_SOURCE_FILE_ID)["valueString"],
            json!("patient.csv:00007")
        );

        let again = meta_for_row(&row, 12345, None);
        assert_eq!(
            extension(&again, constants::EXT_META_SOURCE_FILE_ID)["valueString"],
            json!("patient.csv:12345")
        );
        assert_eq!(again["extension"].as_array().unwrap().len(), 5);
        assert_eq!(
            extension(&meta, constants::EXT_META_SOURCE_FILE_ID)["valueString"],
            json!("patient.csv")
        );
    }

    #[test]
    fn row_meta_adds_the_source_record_id() {
        let meta = create_meta("patient.csv", "Patient", &general());
        let row = meta_for_row(&meta, 1, Some("rec-1"));
        assert_eq!(row["extension"].as_array().unwrap().len(), 6);
        assert_eq!(
            extension(&row, constants::EXT_META_SOURCE_RECORD_ID)["valueString"],
            json!("rec-1")
        );

        let without = meta_for_row(&meta, 1, None);
        assert_eq!(without["extension"].as_array().unwrap().len(), 5);
    }
}
