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
