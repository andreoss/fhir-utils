//! Record field names each converter reads, used to lint contract task targets.

const ALLERGY_INTOLERANCE: &[&str] = &[
    "allergyCategory",
    "allergyClinicalStatusCode",
    "allergyCode",
    "allergyCodeSystem",
    "allergyCodeText",
    "allergyCriticality",
    "allergyManifestationCode",
    "allergyManifestationCodeList",
    "allergyManifestationSystem",
    "allergyManifestationText",
    "allergyOnsetEndDateTime",
    "allergyOnsetStartDateTime",
    "allergyRecordedDateTime",
    "allergyType",
    "allergyVerificationStatusCode",
    "resourceInternalId",
];

const BASIC: &[&str] = &[
    "baseSystem",
    "created_date",
    "otherIdentifierList",
    "patientInternalIdentifier",
    "tokenList",
];

const COMMON: &[&str] = &[
    "configResourceType",
    "encounterInternalId",
    "id",
    "name",
    "nameFirst",
    "nameFirstMiddle",
    "nameFirstMiddleLast",
    "nameLast",
    "nameMiddle",
    "namePrefix",
    "nameSuffix",
    "patientInternalId",
    "prefix",
    "resourceType",
    "suffix",
    "timeZone",
];

const CONDITION: &[&str] = &[
    "conditionCategory",
    "conditionChronicity",
    "conditionClinicalStatus",
    "conditionCode",
    "conditionCodeSystem",
    "conditionCodeText",
    "conditionDiagnosisRank",
    "conditionDiagnosisUse",
    "conditionSeverityCode",
    "conditionSeveritySystem",
    "conditionSeverityText",
    "conditionVerificationStatus",
    "id",
    "resourceInternalId",
];

const ENCOUNTER: &[&str] = &[
    "accountNumber",
    "encounterClaimType",
    "encounterClassCode",
    "encounterClassSystem",
    "encounterClassText",
    "encounterDrgCode",
    "encounterEndDateTime",
    "encounterInsuredCategoryCode",
    "encounterInsuredCategorySystem",
    "encounterInsuredCategoryText",
    "encounterInsuredEntryId",
    "encounterInsuredRank",
    "encounterLengthUnits",
    "encounterLengthValue",
    "encounterLocationPeriodEnd",
    "encounterLocationPeriodStart",
    "encounterLocationSequenceId",
    "encounterParticipantSequenceId",
    "encounterParticipantTypeCode",
    "encounterParticipantTypeCodeSystem",
    "encounterParticipantTypeSystem",
    "encounterParticipantTypeText",
    "encounterPriorityCode",
    "encounterPriorityCodeSystem",
    "encounterPrioritySystem",
    "encounterPriorityText",
    "encounterReasonCode",
    "encounterReasonCodeSystem",
    "encounterReasonCodeText",
    "encounterReasonSystem",
    "encounterReasonText",
    "encounterStartDateTime",
    "encounterStatus",
    "encounterStatusHistory",
    "hospitalizationAdmitSourceCode",
    "hospitalizationAdmitSourceCodeSystem",
    "hospitalizationAdmitSourceCodeText",
    "hospitalizationAdmitSourceSystem",
    "hospitalizationAdmitSourceText",
    "hospitalizationDischargeDispositionCode",
    "hospitalizationDischargeDispositionCodeSystem",
    "hospitalizationDischargeDispositionCodeText",
    "hospitalizationDischargeDispositionSystem",
    "hospitalizationDischargeDispositionText",
    "hospitalizationReAdmissionCode",
    "hospitalizationReAdmissionCodeSystem",
    "hospitalizationReAdmissionCodeText",
    "hospitalizationReAdmissionSystem",
    "hospitalizationReAdmissionText",
    "patientInternalId",
    "practitionerInternalId",
    "practitionerNPI",
    "resourceInternalId",
];

const IMMUNIZATION: &[&str] = &[
    "assigningAuthority",
    "immunizationDate",
    "immunizationDoseQuantity",
    "immunizationDoseUnit",
    "immunizationExpirationDate",
    "immunizationRouteCode",
    "immunizationRouteSystem",
    "immunizationRouteText",
    "immunizationSiteCode",
    "immunizationSiteSystem",
    "immunizationSiteText",
    "immunizationStatus",
    "immunizationStatusReasonCode",
    "immunizationStatusReasonSystem",
    "immunizationStatusReasonText",
    "immunizationVaccineCode",
    "immunizationVaccineCodeList",
    "immunizationVaccineDisplay",
    "immunizationVaccineSystem",
    "immunizationVaccineText",
    "organizationName",
    "resourceInternalId",
];

const LOCATION: &[&str] = &[
    "locationName",
    "locationTypeCode",
    "locationTypeCodeSystem",
    "locationTypeText",
    "resourceInternalId",
];

const MEDICATION: &[&str] = &[
    "assigningAuthority",
    "encounterClaimType",
    "encounterClassCode",
    "medicationAuthoredOn",
    "medicationCode",
    "medicationCodeDisplay",
    "medicationCodeList",
    "medicationCodeSystem",
    "medicationCodeText",
    "medicationQuantity",
    "medicationRefills",
    "medicationRequestIntent",
    "medicationUseCategoryCode",
    "medicationUseCategoryCodeText",
    "medicationUseCategoryText",
    "medicationUseDosageText",
    "medicationUseDosageUnit",
    "medicationUseDosageValue",
    "medicationUseOccuranceDateTime",
    "medicationUseRouteCode",
    "medicationUseRouteCodeSystem",
    "medicationUseRouteList",
    "medicationUseRouteSystem",
    "medicationUseRouteText",
    "medicationUseStatus",
    "medicationValidityEnd",
    "medicationValidityStart",
    "resourceInternalId",
    "resourceType",
];

const OBSERVATION: &[&str] = &[
    "assigningAuthority",
    "observationCategory",
    "observationCode",
    "observationCodeList",
    "observationCodeSystem",
    "observationCodeText",
    "observationDateTime",
    "observationInterpretationCode",
    "observationInterpretationCodeDisplay",
    "observationInterpretationCodeSystem",
    "observationInterpretationCodeText",
    "observationInterpretationDisplay",
    "observationInterpretationSystem",
    "observationInterpretationText",
    "observationRefRange",
    "observationRefRangeHigh",
    "observationRefRangeLow",
    "observationRefRangeText",
    "observationStatus",
    "observationValue",
    "observationValueDataType",
    "observationValueUnits",
    "practitionerInternalId",
    "practitionerNPI",
    "resourceInternalId",
];

const ORGANIZATION: &[&str] = &["organizationName", "resourceInternalId"];

const PATIENT: &[&str] = &[
    "address1",
    "address2",
    "addressText",
    "ageInMonthsForAgeUnder8Years",
    "ageInWeeksForAgeUnder2Years",
    "birthDate",
    "city",
    "country",
    "deceasedBoolean",
    "deceasedDateTime",
    "ethnicity",
    "ethnicitySystem",
    "ethnicityText",
    "gender",
    "multipleBirthBoolean",
    "multipleBirthInteger",
    "patientInternalId",
    "postalCode",
    "race",
    "raceSystem",
    "raceText",
    "state",
    "telecomPhone",
];

const PRACTITIONER: &[&str] = &[
    "identifier_practitionerNPI",
    "practitionerGender",
    "practitionerNameFirst",
    "practitionerNameLast",
    "practitionerNameText",
    "practitionerRoleCode",
    "practitionerRoleCodeList",
    "practitionerRoleCodeSystem",
    "practitionerRoleText",
    "practitionerSpecialtyCode",
    "practitionerSpecialtyCodeList",
    "practitionerSpecialtyCodeSystem",
    "practitionerSpecialtyText",
    "resourceInternalId",
];

const PROCEDURE: &[&str] = &[
    "assigningAuthority",
    "procedureCategory",
    "procedureCategorySystem",
    "procedureCategoryText",
    "procedureCode",
    "procedureCodeDisplay",
    "procedureCodeList",
    "procedureCodeSystem",
    "procedureCodeText",
    "procedureEncounterSequenceId",
    "procedureModifierList",
    "procedureModifierSystem",
    "procedurePerformedDateTime",
    "procedureStatus",
    "resourceInternalId",
    "resourceType",
];

const UNSTRUCTURED: &[&str] = &[
    "documentAttachmentContent",
    "documentAttachmentContentType",
    "documentAttachmentTitle",
    "documentDateTime",
    "documentStatus",
    "documentTypeCode",
    "documentTypeCodeSystem",
    "documentTypeCodeText",
    "resourceInternalId",
    "resourceStatus",
    "resourceType",
];

/// Fields injected by the default task chain or read through the identifier tables.
const INJECTED: &[&str] = &[
    "accountNumber",
    "assigningAuthority",
    "configResourceType",
    "driversLicense",
    "driversLicenseSystem",
    "encounterInternalId",
    "encounterNumber",
    "filePath",
    "identifier_practitionerNPI",
    "medicationRxNumber",
    "mrn",
    "patientInternalId",
    "practitionerNPI",
    "resourceInternalId",
    "rowNum",
    "ssn",
    "ssnSystem",
    "streamType",
    "tenantId",
    "timeZone",
];

const SOURCE_RECORD_ID_SUFFIX: &str = "SourceRecordId";

const MODEL_FIELDS: &[(&str, &[&str])] = &[
    ("Patient", PATIENT),
    ("AllergyIntolerance", ALLERGY_INTOLERANCE),
    ("Condition", CONDITION),
    ("Encounter", ENCOUNTER),
    ("Immunization", IMMUNIZATION),
    ("Observation", OBSERVATION),
    ("Location", LOCATION),
    ("Organization", ORGANIZATION),
    ("Practitioner", PRACTITIONER),
    ("Procedure", PROCEDURE),
    ("MedicationUse", MEDICATION),
    ("MedicationAdministration", MEDICATION),
    ("MedicationRequest", MEDICATION),
    ("MedicationStatement", MEDICATION),
    ("DocumentReference", UNSTRUCTURED),
    ("DiagnosticReport", UNSTRUCTURED),
    ("Unstructured", UNSTRUCTURED),
    ("Basic", BASIC),
];

/// Field names the converter for `resource_type` reads, or `None` for an unknown type.
pub fn fields_for(resource_type: &str) -> Option<&'static [&'static str]> {
    MODEL_FIELDS
        .iter()
        .find(|(key, _)| *key == resource_type)
        .map(|(_, fields)| *fields)
}

/// True when `name` reaches the resource built for `resource_type`.
pub fn is_known(resource_type: &str, name: &str) -> bool {
    if name.ends_with(SOURCE_RECORD_ID_SUFFIX) || COMMON.contains(&name) || INJECTED.contains(&name)
    {
        return true;
    }
    fields_for(resource_type).is_some_and(|fields| fields.contains(&name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    const SOURCES: &[(&str, &[&str])] = &[
        ("patient.rs", PATIENT),
        ("allergy_intolerance.rs", ALLERGY_INTOLERANCE),
        ("condition.rs", CONDITION),
        ("encounter.rs", ENCOUNTER),
        ("immunization.rs", IMMUNIZATION),
        ("observation.rs", OBSERVATION),
        ("location.rs", LOCATION),
        ("organization.rs", ORGANIZATION),
        ("practitioner.rs", PRACTITIONER),
        ("procedure.rs", PROCEDURE),
        ("medication.rs", MEDICATION),
        ("unstructured.rs", UNSTRUCTURED),
        ("basic.rs", BASIC),
    ];

    fn read_keys(file: &str) -> Vec<String> {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/fhirrs")
            .join(file);
        let source = std::fs::read_to_string(path).unwrap();
        let source = source.split("#[cfg(test)]").next().unwrap().to_string();
        let pattern =
            Regex::new(r#"\b(?:field|field_list|field_number|field_bool|get)\s*\(\s*&?\s*[a-z_]+\s*,\s*"([A-Za-z][A-Za-z0-9_]*)""#)
                .unwrap();
        let mut keys: Vec<String> = pattern
            .captures_iter(&source)
            .map(|found| found[1].to_string())
            .collect();
        keys.sort();
        keys.dedup();
        keys
    }

    #[test]
    fn every_field_a_converter_reads_is_registered() {
        for (file, registered) in SOURCES {
            for key in read_keys(file) {
                assert!(
                    registered.contains(&key.as_str()) || COMMON.contains(&key.as_str()),
                    "{file} reads {key}, which the field registry omits"
                );
            }
        }
    }

    #[test]
    fn every_resource_key_maps_to_a_model() {
        for (key, _) in crate::fhirrs::dispatch::RESOURCE_KEYS {
            assert!(fields_for(key).is_some(), "no field list for {key}");
        }
    }

    #[test]
    fn injected_and_source_record_id_columns_are_known() {
        assert!(is_known("Observation", "observationCode"));
        assert!(is_known("Observation", "tenantId"));
        assert!(is_known("Observation", "observationSourceRecordId"));
        assert!(!is_known("Observation", "observationVallue"));
        assert!(!is_known("Patient", "observationCode"));
    }
}
