use crate::contract::Contract;
use crate::error::Error;
use regex::Regex;

pub fn lookup_file_definition<'a>(
    contract: &'a Contract,
    filename: &str,
) -> Result<&'a crate::contract::FileDefinition, Error> {
    if contract.general.regex_filenames {
        for (pattern, def) in &contract.file_definitions {
            let regex = Regex::new(pattern)
                .map_err(|_| Error::Config(format!("invalid regex pattern: {pattern}")))?;
            if regex.is_match(filename) {
                return Ok(def);
            }
        }
    } else {
        for (matcher, def) in &contract.file_definitions {
            if filename.contains(matcher) {
                return Ok(def);
            }
        }
    }

    Err(Error::FileDefinitionNotFound(filename.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{Contract, FileDefinition, FileType, General};
    use std::collections::HashMap;

    fn make_contract(regex_filenames: bool) -> Contract {
        let mut file_definitions = HashMap::new();
        file_definitions.insert(
            "patient".into(),
            FileDefinition {
                file_type: FileType::Csv,
                value_delimiter: ',',
                convert_columns_to_string: true,
                resource_type: "Patient".into(),
                group_by_key: Some("patientInternalId".into()),
                skiprows: None,
                headers: None,
                tasks: None,
                comment: None,
            },
        );
        file_definitions.insert(
            "encounter".into(),
            FileDefinition {
                file_type: FileType::Csv,
                value_delimiter: ',',
                convert_columns_to_string: true,
                resource_type: "Encounter".into(),
                group_by_key: Some("encounterNumber".into()),
                skiprows: None,
                headers: None,
                tasks: None,
                comment: None,
            },
        );

        Contract {
            general: General {
                time_zone: "America/New_York".into(),
                tenant_id: "test".into(),
                stream_type: "live".into(),
                assigning_authority: None,
                empty_field_values: None,
                regex_filenames,
            },
            file_definitions,
        }
    }

    #[test]
    fn test_substring_match() {
        let contract = make_contract(false);
        let def = lookup_file_definition(&contract, "patient_data.csv").unwrap();
        assert_eq!(def.resource_type, "Patient");

        let def = lookup_file_definition(&contract, "encounter_2024.csv").unwrap();
        assert_eq!(def.resource_type, "Encounter");
    }

    #[test]
    fn test_substring_no_match() {
        let contract = make_contract(false);
        let err = lookup_file_definition(&contract, "unknown.csv").unwrap_err();
        assert!(matches!(err, Error::FileDefinitionNotFound(_)));
    }

    #[test]
    fn test_regex_match() {
        let mut contract = make_contract(true);
        contract.file_definitions.clear();
        contract.file_definitions.insert(
            r"patient_\d{4}\.csv".into(),
            FileDefinition {
                file_type: FileType::Csv,
                value_delimiter: ',',
                convert_columns_to_string: true,
                resource_type: "Patient".into(),
                group_by_key: Some("patientInternalId".into()),
                skiprows: None,
                headers: None,
                tasks: None,
                comment: None,
            },
        );

        let def = lookup_file_definition(&contract, "patient_2024.csv").unwrap();
        assert_eq!(def.resource_type, "Patient");
    }

    #[test]
    fn test_regex_no_match() {
        let mut contract = make_contract(true);
        contract.file_definitions.clear();
        contract.file_definitions.insert(
            r"patient_\d{4}\.csv".into(),
            FileDefinition {
                file_type: FileType::Csv,
                value_delimiter: ',',
                convert_columns_to_string: true,
                resource_type: "Patient".into(),
                group_by_key: Some("patientInternalId".into()),
                skiprows: None,
                headers: None,
                tasks: None,
                comment: None,
            },
        );

        let err = lookup_file_definition(&contract, "patient.csv").unwrap_err();
        assert!(matches!(err, Error::FileDefinitionNotFound(_)));
    }
}
