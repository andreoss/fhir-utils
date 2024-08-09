#[cfg(test)]
mod tests {
    use crate::{lookup_file_definition, Contract, Error};
    use std::fs;

    fn read_fixture(name: &str) -> String {
        fs::read_to_string(format!("tests/fixtures/contracts/{name}")).unwrap()
    }

    #[test]
    fn test_load_valid_contract() {
        let json = read_fixture("valid.json");
        let contract = Contract::load(&json).unwrap();
        assert_eq!(contract.general.time_zone, "America/New_York");
        assert_eq!(contract.general.tenant_id, "test-tenant");
        assert_eq!(contract.general.stream_type.to_string(), "live");
        assert_eq!(contract.file_definitions.len(), 2);
    }

    #[test]
    fn test_reject_invalid_timezone() {
        let json = read_fixture("invalid_timezone.json");
        let err = Contract::load(&json).unwrap_err();
        assert!(matches!(err, Error::InvalidTimezone(_)));
    }

    #[test]
    fn test_reject_invalid_stream_type() {
        let json = read_fixture("invalid_stream_type.json");
        let err = Contract::load(&json).unwrap_err();
        assert!(matches!(err, Error::InvalidStreamType(_)));
    }

    #[test]
    fn test_reject_unknown_resource_type() {
        let json = read_fixture("unknown_resource_type.json");
        let err = Contract::load(&json).unwrap_err();
        assert!(matches!(err, Error::UnknownResourceType(_)));
    }

    #[test]
    fn test_reject_fixed_width_no_dict_headers() {
        let json = read_fixture("fixed_width_no_dict_headers.json");
        let err = Contract::load(&json).unwrap_err();
        assert!(matches!(err, Error::FixedWidthRequiresDictHeaders));
    }

    #[test]
    fn test_reject_unknown_task() {
        let json = read_fixture("unknown_task.json");
        let err = Contract::load(&json).unwrap_err();
        assert!(matches!(err, Error::UnknownTask(_)));
    }

    #[test]
    fn test_reject_unknown_task_param() {
        let json = read_fixture("unknown_task_param.json");
        let err = Contract::load(&json).unwrap_err();
        assert!(matches!(err, Error::MissingTaskParam(_)));
    }

    #[test]
    fn test_reject_missing_task_param() {
        let json = read_fixture("missing_task_param.json");
        let err = Contract::load(&json).unwrap_err();
        assert!(matches!(err, Error::MissingTaskParam(_)));
    }

    #[test]
    fn test_reject_empty_tenant_id() {
        let json = read_fixture("empty_tenant_id.json");
        let err = Contract::load(&json).unwrap_err();
        assert!(matches!(err, Error::Config(_)));
    }

    #[test]
    fn test_reject_missing_group_by_key() {
        let json = read_fixture("missing_group_by_key.json");
        let err = Contract::load(&json).unwrap_err();
        assert!(matches!(err, Error::Config(_)));
    }

    #[test]
    fn test_lookup_substring_match() {
        let json = read_fixture("valid.json");
        let contract = Contract::load(&json).unwrap();
        let def = lookup_file_definition(&contract, "patient_data.csv").unwrap();
        assert_eq!(def.resource_type, "Patient");
    }

    #[test]
    fn test_lookup_no_match() {
        let json = read_fixture("valid.json");
        let contract = Contract::load(&json).unwrap();
        let err = lookup_file_definition(&contract, "unknown.csv").unwrap_err();
        assert!(matches!(err, Error::FileDefinitionNotFound(_)));
    }
}
