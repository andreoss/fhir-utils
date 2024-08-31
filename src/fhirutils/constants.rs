pub const HL7_EXTENSION_BASE: &str = "http://hl7.org/fhir/StructureDefinition/";
pub const LOCAL_BASE: &str = "urn:id:";

pub const EXT_RACE: &str = "urn:id:local-race-cd";
pub const EXT_ETHNICITY: &str = "urn:id:ethnicity";
pub const EXT_CHRONICITY: &str = "http://hl7.org/fhir/StructureDefinition/condition-diseaseCourse";
pub const EXT_DATA_ABSENT: &str = "http://hl7.org/fhir/StructureDefinition/data-absent-reason";
pub const EXT_DIAGNOSIS_USE: &str = "urn:id:diagnosis-use-type";
pub const EXT_INSURED: &str = "urn:id:insured";
pub const EXT_INSURED_RANK: &str = "urn:id:insured-rank";
pub const EXT_INSURED_CATEGORY: &str = "urn:id:insured-category";
pub const EXT_CLAIM_TYPE: &str = "urn:id:claim-type";
pub const EXT_META_TENANT_ID: &str = "urn:id:tenant-id";
pub const EXT_META_SOURCE_FILE_ID: &str = "urn:id:source-file-id";
pub const EXT_META_PROCESS_TIMESTAMP: &str = "urn:id:process-timestamp";
pub const EXT_META_SOURCE_EVENT_TRIGGER: &str = "urn:id:source-event-trigger";
pub const EXT_META_SOURCE_RECORD_TYPE: &str = "urn:id:source-record-type";
pub const EXT_META_SOURCE_RECORD_ID: &str = "urn:id:source-record-id";
pub const EXT_AGE_IN_MONTHS: &str = "urn:id:snapshot-age-in-months";
pub const EXT_AGE_IN_WEEKS: &str = "urn:id:snapshot-age-in-weeks";
pub const EXT_PROCEDURE_MODIFIER: &str = "urn:id:procedure-modifier";
pub const EXT_PROCEDURE_SEQUENCE: &str = "urn:id:reference-sequence";

pub const ADMIT_SOURCE_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/admit-source";
pub const ALLERGY_CLINICAL_STATUS_SYSTEM: &str =
    "http://terminology.hl7.org/CodeSystem/allergyintolerance-clinical";
pub const ALLERGY_VERIFICATION_STATUS_SYSTEM: &str =
    "http://terminology.hl7.org/CodeSystem/allergyintolerance-verification";
pub const CLAIM_TYPE_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/claim-type";
pub const CONDITION_CATEGORY_SYSTEM: &str =
    "http://terminology.hl7.org/CodeSystem/condition-category";
pub const CONDITION_CLINICAL_STATUS_SYSTEM: &str =
    "http://terminology.hl7.org/CodeSystem/condition-clinical";
pub const CONDITION_VERIFICATION_STATUS_SYSTEM: &str =
    "http://terminology.hl7.org/CodeSystem/condition-ver-status";
pub const DISCHARGE_DISPOSITION_SYSTEM: &str =
    "http://terminology.hl7.org/CodeSystem/discharge-disposition";
pub const ENCOUNTER_CLASS_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/v3-ActCode";
pub const ETHNICITY_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/v3-Ethnicity";
pub const PROVIDER_TAXONOMY_SYSTEM: &str = "http://nucc.org/provider-taxonomy";
pub const LOCATION_TYPE_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/v3-RoleCode";
pub const MED_ADM_CATEGORY_SYSTEM: &str =
    "http://terminology.hl7.org/CodeSystem/medication-admin-category";
pub const MED_REQ_CATEGORY_SYSTEM: &str =
    "http://terminology.hl7.org/CodeSystem/medicationrequest-category";
pub const MED_STM_CATEGORY_SYSTEM: &str =
    "http://terminology.hl7.org/CodeSystem/medication-statement-category";
pub const OBSERVATION_CATEGORY_SYSTEM: &str =
    "http://terminology.hl7.org/CodeSystem/observation-category";
pub const OBSERVATION_INTERPRETATION_SYSTEM: &str =
    "http://terminology.hl7.org/CodeSystem/v3-ObservationInterpretation";
pub const PARTICIPANT_TYPE_SYSTEM: &str =
    "http://terminology.hl7.org/CodeSystem/v3-ParticipationType";
pub const PRIORITY_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/v3-ActPriority";
pub const RACE_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/v3-Race";
pub const RE_ADMISSION_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/v2-0092";
pub const DATA_ABSENT_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/data-absent-reason";
pub const DRG_CODE_SYSTEM: &str = "urn:id:drg";

pub const CPT_SYSTEM: &str = "http://www.ama-assn.org/go/cpt";
pub const CVX_SYSTEM: &str = "http://hl7.org/fhir/sid/cvx";
pub const ICD9_SYSTEM: &str = "http://hl7.org/fhir/sid/icd-9-cm";
pub const ICD10_SYSTEM: &str = "http://hl7.org/fhir/sid/icd-10-cm";
pub const ICD10PCS_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/icd10PCS";
pub const LOINC_SYSTEM: &str = "http://loinc.org";
pub const MESH_SYSTEM: &str = "https://www.nlm.nih.gov/mesh";
pub const NCI_SYSTEM: &str = "http://ncimeta.nci.nih.gov";
pub const NDC_SYSTEM: &str = "http://hl7.org/fhir/sid/ndc";
pub const RXNORM_SYSTEM: &str = "http://www.nlm.nih.gov/research/umls/rxnorm";
pub const SNOMED_SYSTEM: &str = "http://snomed.info/sct";
pub const UMLS_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/umls";

pub const NPI_SYSTEM: &str = "http://hl7.org.fhir/sid/us-npi";
pub const SSN_SYSTEM: &str = "http://hl7.org/fhir/sid/us-ssn";
pub const IDENTIFIER_TYPE_SYSTEM: &str = "http://terminology.hl7.org/CodeSystem/v2-0203";
pub const IDENTIFIER_TYPE_SYSTEM_RXN: &str = "urn:id:identifier-type";
pub const EXT_ID_SYSTEM: &str = "urn:id:extID";

pub const CODE_SYSTEMS: &[(&str, &str)] = &[
    ("CPT", CPT_SYSTEM),
    ("CVX", CVX_SYSTEM),
    ("ICD9", ICD9_SYSTEM),
    ("ICD10", ICD10_SYSTEM),
    ("ICD10PCS", ICD10PCS_SYSTEM),
    ("LOINC", LOINC_SYSTEM),
    ("MESH", MESH_SYSTEM),
    ("NCI", NCI_SYSTEM),
    ("NDC", NDC_SYSTEM),
    ("RXNORM", RXNORM_SYSTEM),
    ("SNOMED", SNOMED_SYSTEM),
    ("UMLS", UMLS_SYSTEM),
];

pub const RACE_DISPLAY: &[(&str, &str)] = &[
    ("1002-5", "American Indian or Alaska Native"),
    ("1004-1", "American Indian"),
    ("2028-9", "Asian"),
    ("2029-7", "Asian Indian"),
    ("2030-5", "Bangladeshi"),
    ("2031-3", "Bhutanese"),
    ("2032-1", "Burmese"),
    ("2033-9", "Cambodian"),
    ("2034-7", "Chinese"),
    ("2035-4", "Taiwanese"),
    ("2036-2", "Filipino"),
    ("2037-0", "Hmong"),
    ("2038-8", "Indonesian"),
    ("2039-6", "Japanese"),
    ("2040-4", "Korean"),
    ("2041-2", "Laotian"),
    ("2042-0", "Malaysian"),
    ("2043-8", "Okinawan"),
    ("2044-6", "Pakistani"),
    ("2045-3", "Sri Lankan"),
    ("2046-1", "Thai"),
    ("2047-9", "Vietnamese"),
    ("2048-7", "Iwo Jiman"),
    ("2049-5", "Maldivian"),
    ("2050-3", "Nepalese"),
    ("2051-1", "Singaporean"),
    ("2052-9", "Madagascar"),
    ("2054-5", "Black or African American"),
    ("2056-0", "Black"),
    ("2058-6", "African American"),
    ("2060-2", "African"),
    ("2067-7", "Bahamian"),
    ("2068-5", "Barbadian"),
    ("2069-3", "Dominican"),
    ("2070-1", "Dominica Islander"),
    ("2071-9", "Haitian"),
    ("2072-7", "Jamaican"),
    ("2073-5", "Tobagoan"),
    ("2074-3", "Trinidadian"),
    ("2075-0", "West Indian"),
    ("2076-8", "Native Hawaiian or Other Pacific Islander"),
    ("2078-4", "Polynesian"),
    ("2500-7", "Other Pacific Islander"),
    ("2106-3", "White"),
    ("2108-9", "European"),
    ("2129-5", "Arab"),
    ("2131-1", "Other Race"),
];

pub const ETHNICITY_DISPLAY: &[(&str, &str)] = &[
    ("2135-2", "Hispanic or Latino"),
    ("2186-5", "Not Hispanic or Latino"),
];

pub const ALLERGY_CLINICAL_STATUS_DISPLAY: &[(&str, &str)] = &[
    ("active", "Active"),
    ("inactive", "Inactive"),
    ("resolved", "Resolved"),
];

pub const ALLERGY_VERIFICATION_STATUS_DISPLAY: &[(&str, &str)] = &[
    ("unconfirmed", "Unconfirmed"),
    ("confirmed", "Confirmed"),
    ("refuted", "Refuted"),
    ("entered-in-error", "Entered in Error"),
];

pub const CONDITION_CATEGORY_DISPLAY: &[(&str, &str)] = &[
    ("problem-list-item", "Problem List Item"),
    ("encounter-diagnosis", "Encounter Diagnosis"),
];

pub const CONDITION_CLINICAL_STATUS_DISPLAY: &[(&str, &str)] = &[
    ("active", "Active"),
    ("recurrence", "Recurrence"),
    ("relapse", "Relapse"),
    ("inactive", "Inactive"),
    ("remission", "Remission"),
    ("resolved", "Resolved"),
];

pub const CONDITION_VERIFICATION_STATUS_DISPLAY: &[(&str, &str)] = &[
    ("unconfirmed", "Unconfirmed"),
    ("provisional", "Provisional"),
    ("differential", "Differential"),
    ("confirmed", "Confirmed"),
    ("refuted", "Refuted"),
    ("entered-in-error", "Entered in Error"),
];

pub const ENCOUNTER_CLASS_DISPLAY: &[(&str, &str)] = &[
    ("IMP", "inpatient encounter"),
    ("EMER", "emergency"),
    ("AMB", "ambulatory"),
    ("RF", "Refill"),
    ("VR", "virtual"),
    ("HH", "home health"),
];

pub const PARTICIPANT_TYPE_DISPLAY: &[(&str, &str)] = &[
    ("ADM", "admitter"),
    ("ATND", "attender"),
    ("CALLBCK", "callback contact"),
    ("CON", "consultant"),
    ("DIS", "discharger"),
    ("ESC", "escort"),
    ("REF", "referrer"),
    ("SPRF", "secondary performer"),
    ("PPRF", "primary performer"),
    ("PART", "Participation"),
];

pub const ADMIT_SOURCE_DISPLAY: &[(&str, &str)] = &[
    ("hosp-trans", "Transferred from other hospital"),
    ("emd", "From accident/emergency department"),
    ("outp", "From outpatient department"),
    ("born", "Born in hospital"),
    ("gp", "General Practitioner referral"),
    ("mp", "Medical Practitioner/physician referral"),
    ("nursing", "From nursing home"),
    ("psych", "From psychiatric hospital"),
    ("rehab", "From rehabilitation facility"),
    ("other", "Other"),
];

pub const DIAGNOSIS_USE_DISPLAY: &[(&str, &str)] = &[
    ("AD", "Admission diagnosis"),
    ("DD", "Discharge diagnosis"),
    ("CC", "Chief complaint"),
    ("CM", "Comorbidity diagnosis"),
    ("pre-op", "pre-op diagnosis"),
    ("post-op", "post-op diagnosis"),
    ("billing", "Billing"),
];

pub const IMMUNIZATION_STATUS_REASON_DISPLAY: &[(&str, &str)] = &[
    ("IMMUNE", "immunity"),
    ("MEDPREC", "medical precaution"),
    ("OSTOCK", "product out of stock"),
    ("PATOBJ", "patient objection"),
    ("PHILISOP", "philosophica objection"),
    ("RELIG", "religious objection"),
    ("VACEFF", "vaccine efficacy concerns"),
    ("VACSAF", "vaccine safety concerns"),
];

pub const LOCATION_TYPE_DISPLAY: &[(&str, &str)] = &[
    ("ER", "Emergency room"),
    ("HOSP", "Hospital"),
    ("ICU", "Intensive care unit"),
];

pub const OBSERVATION_INTERPRETATION_DISPLAY: &[(&str, &str)] = &[
    ("A", "Abnormal"),
    ("AA", "Critical abnormal"),
    ("H", "High"),
    ("L", "Low"),
    ("N", "Normal"),
    ("NEG", "Negative"),
    ("POS", "Positive"),
];

pub const OBSERVATION_CATEGORY_DISPLAY: &[(&str, &str)] = &[
    ("social-history", "Social History"),
    ("vital-signs", "Vital Signs"),
    ("imaging", "Imaging"),
    ("laboratory", "Laboratory"),
    ("procedure", "Procedure"),
    ("survey", "Survey"),
    ("exam", "Exam"),
    ("therapy", "Therapy"),
    ("activity", "Activity"),
];

pub fn display(table: &[(&str, &'static str)], code: &str) -> Option<&'static str> {
    table
        .iter()
        .find(|(key, _)| *key == code)
        .map(|(_, display)| *display)
}

pub fn system_url(short_name: &str) -> Option<&'static str> {
    CODE_SYSTEMS
        .iter()
        .find(|(name, _)| *name == short_name)
        .map(|(_, url)| *url)
}

pub fn system_short_name(url: &str) -> Option<&'static str> {
    CODE_SYSTEMS
        .iter()
        .find(|(_, system)| *system == url)
        .map(|(name, _)| *name)
}
