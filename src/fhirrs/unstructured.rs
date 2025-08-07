use crate::error::Error;
use crate::fhirrs::common;
use crate::fhirrs::practitioner;
use crate::fhirutils::builders;
use crate::fhirutils::constants;
use crate::fhirutils::identifiers;
use serde_json::{json, Value};

pub const DOCUMENT_REFERENCE: &str = "DocumentReference";
pub const DIAGNOSTIC_REPORT: &str = "DiagnosticReport";

const DEFAULT_STATUS: &str = "current";

pub fn convert_record(
    group_by_key: &str,
    record: &Value,
    meta: &Value,
) -> Result<Vec<Value>, Error> {
    let kind = builders::field(record, "resourceType")
        .filter(|kind| *kind == DIAGNOSTIC_REPORT)
        .unwrap_or(DOCUMENT_REFERENCE);
    build(kind, group_by_key, record, meta)
}

pub fn convert_document_reference(
    group_by_key: &str,
    record: &Value,
    meta: &Value,
) -> Result<Vec<Value>, Error> {
    build(DOCUMENT_REFERENCE, group_by_key, record, meta)
}

pub fn convert_diagnostic_report(
    group_by_key: &str,
    record: &Value,
    meta: &Value,
) -> Result<Vec<Value>, Error> {
    build(DIAGNOSTIC_REPORT, group_by_key, record, meta)
}

fn build(
    kind: &str,
    group_by_key: &str,
    record: &Value,
    meta: &Value,
) -> Result<Vec<Value>, Error> {
    let record = common::normalized(record);
    let zone = common::zone(&record);
    let attachment = builders::attachment(
        builders::field(&record, "documentAttachmentContentType"),
        builders::field(&record, "documentAttachmentContent"),
        builders::field(&record, "documentAttachmentTitle"),
        builders::field(&record, "documentDateTime"),
        zone,
    );
    let attachment = match attachment {
        Some(attachment) => attachment,
        None => return Ok(Vec::new()),
    };

    let mut resource = common::resource(
        kind,
        &builders::resource_id(builders::field(&record, "resourceInternalId")),
        meta,
    );

    let code = match builders::field(&record, "documentTypeCode") {
        Some(code) => builders::codeable_concept(
            Some(
                builders::field(&record, "documentTypeCodeSystem")
                    .unwrap_or(constants::LOINC_SYSTEM),
            ),
            Some(code),
            None,
            builders::field(&record, "documentTypeCodeText"),
        ),
        None => default_document_code(kind),
    };

    let mut identifier_values = identifiers::identifier_list(&record).unwrap_or_default();
    if let Some(creation) = attachment.get("creation").and_then(|value| value.as_str()) {
        let code_value = code
            .as_ref()
            .and_then(|concept| concept.get("coding"))
            .and_then(|codings| codings.get(0))
            .and_then(|coding| coding.get("code"))
            .and_then(|code| code.as_str());
        if let Some(code_value) = code_value {
            if let Some(external) =
                identifiers::build_extid_identifier(&format!("{code_value}-{creation}"))
            {
                identifier_values.push(external);
            }
        }
    }
    common::insert_list(&mut resource, "identifier", identifier_values);

    common::insert(
        &mut resource,
        "subject",
        common::subject_reference(&record, group_by_key),
    );

    let practitioners = practitioner::convert_record(group_by_key, &record, meta)?;
    let practitioner_reference = practitioners.first().and_then(common::reference_to);
    let mut resources = practitioners;

    let issued = builders::field(&record, "documentDateTime")
        .and_then(|value| builders::datetime(value, zone));
    let encounter_reference = common::encounter_reference(&record);

    if kind == DIAGNOSTIC_REPORT {
        common::insert_str(
            &mut resource,
            "status",
            builders::field(&record, "documentStatus"),
        );
        common::insert(&mut resource, "code", code);
        common::insert(&mut resource, "encounter", encounter_reference);
        if let Some(issued) = issued {
            resource.insert("issued".into(), json!(issued));
        }
        if let Some(practitioner_reference) = practitioner_reference {
            common::insert_list(
                &mut resource,
                "resultsInterpreter",
                vec![practitioner_reference],
            );
        }
        common::insert_list(&mut resource, "presentedForm", vec![attachment]);
    } else {
        resource.insert(
            "status".into(),
            json!(builders::field(&record, "resourceStatus").unwrap_or(DEFAULT_STATUS)),
        );
        common::insert_str(
            &mut resource,
            "docStatus",
            builders::field(&record, "documentStatus"),
        );
        common::insert(&mut resource, "type", code);
        if let Some(issued) = issued {
            resource.insert("date".into(), json!(issued));
        }
        if let Some(encounter_reference) = encounter_reference {
            resource.insert(
                "context".into(),
                json!({"encounter": [encounter_reference]}),
            );
        }
        if let Some(practitioner_reference) = practitioner_reference {
            common::insert_list(&mut resource, "author", vec![practitioner_reference]);
        }
        common::insert_list(
            &mut resource,
            "content",
            vec![json!({"attachment": attachment})],
        );
    }

    resources.push(Value::Object(resource));
    Ok(resources)
}

fn default_document_code(kind: &str) -> Option<Value> {
    let (code, text) = if kind == DIAGNOSTIC_REPORT {
        ("50398-7", "Narrative diagnostic report [Interpretation]")
    } else {
        ("67781-5", "Summarization of encounter note Narrative")
    };
    builders::codeable_concept(Some(constants::LOINC_SYSTEM), Some(code), Some(text), None)
}

#[cfg(test)]
mod tests {
    use crate::fhirrs::unstructured::*;
    use crate::fhirutils::constants;
    use serde_json::{json, Value};

    fn meta() -> Value {
        json!({"extension": []})
    }

    #[test]
    fn no_attachment_content_produces_no_resource() {
        let record = json!({"patientInternalId": "p1", "documentTypeCode": "11488-4"});
        assert!(convert_record("g1", &record, &meta()).unwrap().is_empty());
    }

    #[test]
    fn unstructured_defaults_to_a_document_reference() {
        let record = json!({
            "resourceInternalId": "d1",
            "patientInternalId": "p1",
            "encounterInternalId": "e1",
            "documentAttachmentContent": "note text",
            "documentAttachmentTitle": "Progress note",
            "documentDateTime": "2021-06-01 10:00:00",
            "documentStatus": "final",
            "timeZone": "UTC"
        });

        let document = convert_record("g1", &record, &meta()).unwrap().remove(0);
        assert_eq!(document["resourceType"], json!("DocumentReference"));
        assert_eq!(document["id"], json!("d1"));
        assert_eq!(document["status"], json!("current"));
        assert_eq!(document["docStatus"], json!("final"));
        assert_eq!(document["subject"], json!({"reference": "Patient/p1"}));
        assert_eq!(document["date"], json!("2021-06-01T10:00:00+00:00"));
        assert_eq!(
            document["context"]["encounter"][0],
            json!({"reference": "Encounter/e1"})
        );
        assert_eq!(
            document["content"][0]["attachment"]["data"],
            json!("bm90ZSB0ZXh0")
        );
        assert_eq!(document["type"]["coding"][0]["code"], json!("67781-5"));
        assert_eq!(
            document["type"]["coding"][0]["system"],
            json!(constants::LOINC_SYSTEM)
        );

        let ext_id = document["identifier"]
            .as_array()
            .unwrap()
            .iter()
            .find(|identifier| identifier["id"] == json!("extID"))
            .unwrap();
        assert_eq!(ext_id["value"], json!("67781-5-2021-06-01T10:00:00+00:00"));
    }

    #[test]
    fn diagnostic_reports_use_presented_form_and_results_interpreter() {
        let record = json!({
            "resourceInternalId": "d1",
            "documentAttachmentContent": "report",
            "documentAttachmentContentType": "text/html",
            "documentStatus": "final",
            "documentDateTime": "2021-06-01",
            "practitionerInternalId": "pr1",
            "practitionerNameLast": "House",
            "timeZone": "UTC"
        });

        let resources = convert_diagnostic_report("g1", &record, &meta()).unwrap();
        let report = resources.last().unwrap();
        assert_eq!(report["resourceType"], json!("DiagnosticReport"));
        assert_eq!(report["status"], json!("final"));
        assert_eq!(report["issued"], json!("2021-06-01T00:00:00+00:00"));
        assert_eq!(
            report["presentedForm"][0]["contentType"],
            json!("text/html")
        );
        assert_eq!(report["code"]["coding"][0]["code"], json!("50398-7"));
        assert_eq!(
            report["resultsInterpreter"][0]["reference"],
            json!("Practitioner/pr1")
        );
        assert!(resources
            .iter()
            .any(|resource| resource["resourceType"] == json!("Practitioner")));
    }

    #[test]
    fn resource_type_field_selects_the_diagnostic_report() {
        let record = json!({
            "resourceType": "DiagnosticReport",
            "documentAttachmentContent": "report"
        });
        let report = convert_record("g1", &record, &meta()).unwrap().remove(0);
        assert_eq!(report["resourceType"], json!("DiagnosticReport"));
    }

    #[test]
    fn explicit_document_codes_win_over_defaults() {
        let record = json!({
            "documentAttachmentContent": "note",
            "documentTypeCode": "11488-4",
            "documentTypeCodeText": "Consult note"
        });
        let document = convert_document_reference("g1", &record, &meta())
            .unwrap()
            .remove(0);
        assert_eq!(document["type"]["coding"][0]["code"], json!("11488-4"));
        assert_eq!(document["type"]["text"], json!("Consult note"));
    }
}
