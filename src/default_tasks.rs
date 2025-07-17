use crate::contract::{Contract, FileDefinition, General};
use crate::error::Error;
use crate::reader::RecordBatch;
use crate::tasks::{execute_task_chain, TaskRegistry};
use serde_json::json;
use std::collections::HashMap;

pub fn build_default_task_chain(
    general: &General,
    file_def: &FileDefinition,
    file_path: &str,
    config_resource_type: &str,
) -> Vec<crate::contract::Task> {
    build_default_task_chain_with_start(general, file_def, file_path, config_resource_type, 1)
}

pub fn build_default_task_chain_with_start(
    general: &General,
    file_def: &FileDefinition,
    file_path: &str,
    config_resource_type: &str,
    starting_row_num: usize,
) -> Vec<crate::contract::Task> {
    let mut tasks = Vec::new();

    tasks.push(crate::contract::Task {
        task: "add_row_num".into(),
        params: HashMap::from([("starting_index".into(), json!(starting_row_num))]),
    });

    tasks.push(crate::contract::Task {
        task: "set_nan_to_none".into(),
        params: HashMap::new(),
    });

    tasks.push(crate::contract::Task {
        task: "remove_whitespace_from_columns".into(),
        params: HashMap::new(),
    });

    if let Some(group_by_key) = &file_def.group_by_key {
        tasks.push(crate::contract::Task {
            task: "copy_columns".into(),
            params: HashMap::from([
                ("columns".into(), json!([group_by_key])),
                ("target_column".into(), json!("groupByKey")),
                ("value_separator".into(), json!("|")),
            ]),
        });
    }

    tasks.push(crate::contract::Task {
        task: "add_constant".into(),
        params: HashMap::from([
            ("name".into(), json!("timeZone")),
            ("value".into(), json!(general.time_zone)),
        ]),
    });

    tasks.push(crate::contract::Task {
        task: "add_constant".into(),
        params: HashMap::from([
            ("name".into(), json!("tenantId")),
            ("value".into(), json!(general.tenant_id)),
        ]),
    });

    tasks.push(crate::contract::Task {
        task: "add_constant".into(),
        params: HashMap::from([
            ("name".into(), json!("streamType")),
            ("value".into(), json!(general.stream_type)),
        ]),
    });

    if let Some(assigning_authority) = &general.assigning_authority {
        tasks.push(crate::contract::Task {
            task: "add_constant".into(),
            params: HashMap::from([
                ("name".into(), json!("assigningAuthority")),
                ("value".into(), json!(assigning_authority)),
            ]),
        });
    }

    if let Some(empty_field_values) = &general.empty_field_values {
        tasks.push(crate::contract::Task {
            task: "add_constant".into(),
            params: HashMap::from([
                ("name".into(), json!("emptyFieldValues")),
                ("value".into(), json!(empty_field_values.join(","))),
            ]),
        });
    }

    tasks.push(crate::contract::Task {
        task: "add_constant".into(),
        params: HashMap::from([
            ("name".into(), json!("regexFilenames")),
            ("value".into(), json!(general.regex_filenames.to_string())),
        ]),
    });

    tasks.push(crate::contract::Task {
        task: "add_constant".into(),
        params: HashMap::from([
            ("name".into(), json!("filePath")),
            ("value".into(), json!(file_path)),
        ]),
    });

    tasks.push(crate::contract::Task {
        task: "add_constant".into(),
        params: HashMap::from([
            ("name".into(), json!("configResourceType")),
            ("value".into(), json!(config_resource_type)),
        ]),
    });

    tasks
}

pub fn execute_default_and_user_tasks(
    batch: &mut RecordBatch,
    contract: &Contract,
    file_def: &FileDefinition,
    file_path: &str,
    registry: &TaskRegistry,
) -> Vec<Error> {
    let default_tasks = build_default_task_chain(
        &contract.general,
        file_def,
        file_path,
        &file_def.resource_type,
    );

    let mut all_tasks = default_tasks;
    if let Some(user_tasks) = &file_def.tasks {
        all_tasks.extend(user_tasks.clone());
    }

    execute_task_chain(batch, &all_tasks, registry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{Contract, FileDefinition, FileType, General, Headers, Task};
    use crate::reader::RecordBatch;
    use crate::tasks::TaskRegistry;
    use serde_json::json;
    use std::collections::HashMap;

    fn make_general() -> General {
        General {
            time_zone: "America/New_York".into(),
            tenant_id: "test-tenant".into(),
            stream_type: "live".into(),
            assigning_authority: Some("1.2.3.4.5".into()),
            empty_field_values: Some(vec!["".into(), "NA".into(), "N/A".into(), "null".into()]),
            regex_filenames: false,
        }
    }

    fn make_file_def() -> FileDefinition {
        FileDefinition {
            file_type: FileType::Csv,
            value_delimiter: ',',
            convert_columns_to_string: true,
            resource_type: "Patient".into(),
            group_by_key: Some("patientInternalId".into()),
            skiprows: None,
            headers: Some(Headers::List(vec![
                "patientInternalId".into(),
                "nameFirst".into(),
                "nameLast".into(),
            ])),
            tasks: None,
            comment: None,
        }
    }

    fn make_contract() -> Contract {
        Contract {
            general: make_general(),
            file_definitions: HashMap::new(),
        }
    }

    fn make_batch() -> RecordBatch {
        let mut columns = HashMap::new();
        columns.insert(
            "patientInternalId".into(),
            vec![Some("123".into()), Some("456".into())],
        );
        columns.insert(
            "nameFirst".into(),
            vec![Some("John".into()), Some("Jane".into())],
        );
        columns.insert(
            "nameLast".into(),
            vec![Some("Doe".into()), Some("Smith".into())],
        );
        RecordBatch {
            columns,
            row_count: 2,
        }
    }

    #[test]
    fn test_build_default_task_chain() {
        let general = make_general();
        let file_def = make_file_def();
        let tasks = build_default_task_chain(&general, &file_def, "/path/to/file.csv", "Patient");

        let task_names: Vec<&str> = tasks.iter().map(|t| t.task.as_str()).collect();
        assert_eq!(
            task_names,
            vec![
                "add_row_num",
                "set_nan_to_none",
                "remove_whitespace_from_columns",
                "copy_columns",
                "add_constant",
                "add_constant",
                "add_constant",
                "add_constant",
                "add_constant",
                "add_constant",
                "add_constant",
                "add_constant",
            ]
        );

        assert_eq!(tasks[0].params.get("starting_index"), Some(&json!(1)));
        assert_eq!(
            tasks[3].params.get("columns"),
            Some(&json!(["patientInternalId"]))
        );
        assert_eq!(
            tasks[3].params.get("target_column"),
            Some(&json!("groupByKey"))
        );

        let constant_names: Vec<&str> = tasks[4..]
            .iter()
            .filter_map(|t| t.params.get("name").and_then(|v| v.as_str()))
            .collect();
        assert!(constant_names.contains(&"timeZone"));
        assert!(constant_names.contains(&"tenantId"));
        assert!(constant_names.contains(&"streamType"));
        assert!(constant_names.contains(&"assigningAuthority"));
        assert!(constant_names.contains(&"emptyFieldValues"));
        assert!(constant_names.contains(&"regexFilenames"));
        assert!(constant_names.contains(&"filePath"));
        assert!(constant_names.contains(&"configResourceType"));
    }

    #[test]
    fn test_build_default_task_chain_without_optional_fields() {
        let mut general = make_general();
        general.assigning_authority = None;
        general.empty_field_values = None;

        let file_def = make_file_def();
        let tasks = build_default_task_chain(&general, &file_def, "/path/to/file.csv", "Patient");

        let constant_names: Vec<&str> = tasks
            .iter()
            .filter(|t| t.task == "add_constant")
            .filter_map(|t| t.params.get("name").and_then(|v| v.as_str()))
            .collect();

        assert!(constant_names.contains(&"timeZone"));
        assert!(constant_names.contains(&"tenantId"));
        assert!(constant_names.contains(&"streamType"));
        assert!(!constant_names.contains(&"assigningAuthority"));
        assert!(!constant_names.contains(&"emptyFieldValues"));
        assert!(constant_names.contains(&"regexFilenames"));
        assert!(constant_names.contains(&"filePath"));
        assert!(constant_names.contains(&"configResourceType"));
    }

    #[test]
    fn test_execute_default_and_user_tasks() {
        let mut batch = make_batch();
        let contract = make_contract();
        let file_def = make_file_def();
        let registry = TaskRegistry::new();

        let errors = execute_default_and_user_tasks(
            &mut batch,
            &contract,
            &file_def,
            "/path/to/file.csv",
            &registry,
        );
        assert!(errors.is_empty());

        assert_eq!(
            batch.get_column("rowNum").unwrap(),
            &vec![Some("1".into()), Some("2".into())]
        );
        assert_eq!(
            batch.get_column("groupByKey").unwrap(),
            &vec![Some("123".into()), Some("456".into())]
        );
        assert_eq!(
            batch.get_column("timeZone").unwrap(),
            &vec![
                Some("America/New_York".into()),
                Some("America/New_York".into())
            ]
        );
        assert_eq!(
            batch.get_column("tenantId").unwrap(),
            &vec![Some("test-tenant".into()), Some("test-tenant".into())]
        );
        assert_eq!(
            batch.get_column("streamType").unwrap(),
            &vec![Some("live".into()), Some("live".into())]
        );
        assert_eq!(
            batch.get_column("assigningAuthority").unwrap(),
            &vec![Some("1.2.3.4.5".into()), Some("1.2.3.4.5".into())]
        );
        assert_eq!(
            batch.get_column("filePath").unwrap(),
            &vec![
                Some("/path/to/file.csv".into()),
                Some("/path/to/file.csv".into())
            ]
        );
        assert_eq!(
            batch.get_column("configResourceType").unwrap(),
            &vec![Some("Patient".into()), Some("Patient".into())]
        );
    }

    #[test]
    fn test_execute_default_and_user_tasks_with_user_tasks() {
        let mut batch = make_batch();
        let contract = make_contract();
        let mut file_def = make_file_def();
        file_def.tasks = Some(vec![Task {
            task: "add_constant".into(),
            params: HashMap::from([
                ("name".into(), json!("userAdded")),
                ("value".into(), json!("userValue")),
            ]),
        }]);
        let registry = TaskRegistry::new();

        let errors = execute_default_and_user_tasks(
            &mut batch,
            &contract,
            &file_def,
            "/path/to/file.csv",
            &registry,
        );
        assert!(errors.is_empty());

        assert_eq!(
            batch.get_column("userAdded").unwrap(),
            &vec![Some("userValue".into()), Some("userValue".into())]
        );
    }

    #[test]
    fn test_execute_default_and_user_tasks_error_isolation() {
        let mut batch = make_batch();
        let contract = make_contract();
        let mut file_def = make_file_def();
        file_def.tasks = Some(vec![
            Task {
                task: "add_constant".into(),
                params: HashMap::from([
                    ("name".into(), json!("good")),
                    ("value".into(), json!("val")),
                ]),
            },
            Task {
                task: "unknown_task".into(),
                params: HashMap::new(),
            },
            Task {
                task: "add_constant".into(),
                params: HashMap::from([
                    ("name".into(), json!("after_error")),
                    ("value".into(), json!("val")),
                ]),
            },
        ]);
        let registry = TaskRegistry::new();

        let errors = execute_default_and_user_tasks(
            &mut batch,
            &contract,
            &file_def,
            "/path/to/file.csv",
            &registry,
        );
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], Error::UnknownTask(_)));

        assert_eq!(
            batch.get_column("good").unwrap(),
            &vec![Some("val".into()), Some("val".into())]
        );
        assert_eq!(
            batch.get_column("after_error").unwrap(),
            &vec![Some("val".into()), Some("val".into())]
        );
    }
}
