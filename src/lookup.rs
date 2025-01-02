use crate::contract::Contract;
use crate::error::Error;
use regex::Regex;

pub fn lookup_file_definition<'a>(
    contract: &'a Contract,
    filename: &str,
) -> Result<&'a crate::contract::FileDefinition, Error> {
    matched_definition(contract, filename).map(|(_, definition)| definition)
}

/// The matching definition and the contract key that selected it.
pub fn matched_definition<'a>(
    contract: &'a Contract,
    filename: &str,
) -> Result<(&'a str, &'a crate::contract::FileDefinition), Error> {
    for matcher in ordered_matchers(contract) {
        let definition = &contract.file_definitions[matcher];
        let matched = if contract.general.regex_filenames {
            Regex::new(matcher)
                .map_err(|_| Error::Config(format!("invalid regex pattern: {matcher}")))?
                .is_match(filename)
        } else {
            filename.contains(matcher)
        };
        if matched {
            return Ok((matcher, definition));
        }
    }

    Err(Error::FileDefinitionNotFound(filename.into()))
}

fn ordered_matchers(contract: &Contract) -> Vec<&String> {
    let mut matchers: Vec<&String> = contract.file_definitions.keys().collect();
    matchers.sort_by(|left, right| {
        right
            .len()
            .cmp(&left.len())
            .then_with(|| left.as_str().cmp(right.as_str()))
    });
    matchers
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
                extra: Default::default(),
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

    #[test]
    fn the_longest_matcher_wins_and_stays_stable() {
        let mut contract = make_contract(false);
        contract.file_definitions.insert(
            "patient_encounter".into(),
            FileDefinition {
                file_type: FileType::Csv,
                value_delimiter: ',',
                convert_columns_to_string: true,
                resource_type: "Condition".into(),
                group_by_key: Some("patientInternalId".into()),
                skiprows: None,
                headers: None,
                tasks: None,
                comment: None,
            },
        );

        for _ in 0..25 {
            let definition = lookup_file_definition(&contract, "patient_encounter_2021").unwrap();
            assert_eq!(definition.resource_type, "Condition");
        }
        assert_eq!(
            lookup_file_definition(&contract, "patient_2021")
                .unwrap()
                .resource_type,
            "Patient"
        );
    }
}
