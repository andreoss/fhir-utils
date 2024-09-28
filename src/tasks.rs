use crate::error::Error;
use crate::reader::RecordBatch;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub type TaskFn =
    Arc<dyn Fn(&mut RecordBatch, &HashMap<String, Value>) -> Result<(), Error> + Send + Sync>;

#[derive(Clone)]
pub struct TaskRegistry {
    tasks: HashMap<String, TaskFn>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            tasks: HashMap::new(),
        };
        registry.register_builtins();
        registry
    }

    fn register_builtins(&mut self) {
        self.register("add_constant", Arc::new(add_constant));
        self.register("add_row_num", Arc::new(add_row_num));
        self.register("set_nan_to_none", Arc::new(set_nan_to_none));
        self.register(
            "remove_whitespace_from_columns",
            Arc::new(remove_whitespace_from_columns),
        );
        self.register("copy_columns", Arc::new(copy_columns));
        crate::task_library::register(self);
    }

    pub fn register(&mut self, name: &str, func: TaskFn) {
        self.tasks.insert(name.to_string(), func);
    }

    pub fn get(&self, name: &str) -> Option<&TaskFn> {
        self.tasks.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.tasks.contains_key(name)
    }

    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.tasks.keys().cloned().collect();
        names.sort();
        names
    }
}

impl Default for TaskRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub fn execute_task_chain(
    batch: &mut RecordBatch,
    tasks: &[crate::contract::Task],
    registry: &TaskRegistry,
) -> Vec<Error> {
    let mut errors = Vec::new();
    for task in tasks {
        if let Some(task_fn) = registry.get(&task.task) {
            let params = task.params.clone();
            if let Err(e) = task_fn(batch, &params) {
                errors.push(e);
            }
        } else {
            errors.push(Error::UnknownTask(task.task.clone()));
        }
    }
    errors
}

fn add_constant(batch: &mut RecordBatch, params: &HashMap<String, Value>) -> Result<(), Error> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::MissingTaskParam("add_constant.name".into()))?;
    let value = params
        .get("value")
        .ok_or_else(|| Error::MissingTaskParam("add_constant.value".into()))?;
    let value = constant_value(value)
        .ok_or_else(|| Error::MissingTaskParam("add_constant.value".into()))?;

    let column = batch.columns.entry(name.to_string()).or_default();
    column.resize(batch.row_count, Some(value));
    Ok(())
}

fn constant_value(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::Array(items) => {
            let items: Vec<String> = items.iter().filter_map(constant_value).collect();
            crate::task_library::join_list(&items)
        }
        _ => None,
    }
}

fn add_row_num(batch: &mut RecordBatch, params: &HashMap<String, Value>) -> Result<(), Error> {
    let starting_index = params
        .get("starting_index")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Error::MissingTaskParam("add_row_num.starting_index".into()))?
        as usize;

    let column = batch.columns.entry("rowNum".to_string()).or_default();
    column.clear();
    for i in 0..batch.row_count {
        column.push(Some((starting_index + i).to_string()));
    }
    Ok(())
}

fn set_nan_to_none(batch: &mut RecordBatch, _params: &HashMap<String, Value>) -> Result<(), Error> {
    for column in batch.columns.values_mut() {
        for value in column.iter_mut() {
            if value.as_ref().is_some_and(|value| value.trim().is_empty()) {
                *value = None;
            }
        }
    }
    Ok(())
}

fn remove_whitespace_from_columns(
    batch: &mut RecordBatch,
    _params: &HashMap<String, Value>,
) -> Result<(), Error> {
    for column in batch.columns.values_mut() {
        for value in column.iter_mut() {
            if let Some(v) = value {
                *value = Some(v.trim().to_string());
            }
        }
    }

    let renames: Vec<(String, String)> = batch
        .columns
        .keys()
        .filter(|name| name.trim() != name.as_str())
        .map(|name| (name.clone(), name.trim().to_string()))
        .collect();
    for (from, to) in renames {
        if let Some(values) = batch.columns.remove(&from) {
            batch.columns.insert(to, values);
        }
    }
    Ok(())
}

fn copy_columns(batch: &mut RecordBatch, params: &HashMap<String, Value>) -> Result<(), Error> {
    let columns = params
        .get("columns")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Error::MissingTaskParam("copy_columns.columns".into()))?;
    let target_column = params
        .get("target_column")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::MissingTaskParam("copy_columns.target_column".into()))?;
    let value_separator = params
        .get("value_separator")
        .and_then(|v| v.as_str())
        .unwrap_or("|");

    let source_columns: Vec<String> = columns
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    let matched: Vec<&String> = source_columns
        .iter()
        .filter(|name| batch.columns.contains_key(*name))
        .collect();

    let mut target = Vec::with_capacity(batch.row_count);
    for row_idx in 0..batch.row_count {
        let parts: Vec<String> = matched
            .iter()
            .map(
                |name| match batch.columns.get(*name).and_then(|col| col.get(row_idx)) {
                    Some(Some(value)) => value.clone(),
                    _ => String::new(),
                },
            )
            .collect();
        target.push(if parts.iter().all(|part| part.is_empty()) {
            None
        } else {
            Some(parts.join(value_separator))
        });
    }
    batch.columns.insert(target_column.to_string(), target);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::Task;
    use crate::reader::RecordBatch;
    use serde_json::json;
    use std::collections::HashMap;

    fn make_batch_with_columns(columns: HashMap<String, Vec<Option<String>>>) -> RecordBatch {
        let row_count = columns.values().next().map(|v| v.len()).unwrap_or(0);
        RecordBatch { columns, row_count }
    }

    #[test]
    fn test_registry_contains_builtins() {
        let registry = TaskRegistry::new();
        assert!(registry.contains("add_constant"));
        assert!(registry.contains("add_row_num"));
        assert!(registry.contains("set_nan_to_none"));
        assert!(registry.contains("remove_whitespace_from_columns"));
        assert!(registry.contains("copy_columns"));
        assert!(!registry.contains("unknown_task"));
    }

    #[test]
    fn test_add_constant() {
        let mut batch = make_batch_with_columns(HashMap::new());
        batch.row_count = 3;
        let mut params = HashMap::new();
        params.insert("name".into(), json!("test_col"));
        params.insert("value".into(), json!("test_val"));

        add_constant(&mut batch, &params).unwrap();
        assert_eq!(
            batch.get_column("test_col").unwrap(),
            &vec![Some("test_val".into()); 3]
        );
    }

    #[test]
    fn test_add_row_num() {
        let mut batch = make_batch_with_columns(HashMap::new());
        batch.row_count = 3;
        let mut params = HashMap::new();
        params.insert("starting_index".into(), json!(5));

        add_row_num(&mut batch, &params).unwrap();
        assert_eq!(
            batch.get_column("rowNum").unwrap(),
            &vec![Some("5".into()), Some("6".into()), Some("7".into())]
        );
    }

    #[test]
    fn test_set_nan_to_none() {
        let mut batch = make_batch_with_columns(HashMap::from([(
            "col1".into(),
            vec![
                Some("value".into()),
                Some("".into()),
                Some("  ".into()),
                Some("NA".into()),
            ],
        )]));
        let params = HashMap::new();

        set_nan_to_none(&mut batch, &params).unwrap();
        assert_eq!(
            batch.get_column("col1").unwrap(),
            &vec![Some("value".into()), None, None, Some("NA".into())]
        );
    }

    #[test]
    fn test_remove_whitespace_from_columns() {
        let mut batch = make_batch_with_columns(HashMap::from([(
            "col1".into(),
            vec![
                Some("  value  ".into()),
                Some("\ttab\t".into()),
                Some("normal".into()),
            ],
        )]));
        let params = HashMap::new();

        remove_whitespace_from_columns(&mut batch, &params).unwrap();
        assert_eq!(
            batch.get_column("col1").unwrap(),
            &vec![
                Some("value".into()),
                Some("tab".into()),
                Some("normal".into())
            ]
        );
    }

    #[test]
    fn test_add_constant_accepts_non_string_values() {
        let mut batch = make_batch_with_columns(HashMap::new());
        batch.row_count = 2;

        add_constant(
            &mut batch,
            &HashMap::from([("name".into(), json!("count")), ("value".into(), json!(3))]),
        )
        .unwrap();
        add_constant(
            &mut batch,
            &HashMap::from([
                ("name".into(), json!("codes")),
                ("value".into(), json!(["a", "b"])),
            ]),
        )
        .unwrap();

        assert_eq!(
            batch.get_column("count").unwrap(),
            &vec![Some("3".into()); 2]
        );
        assert_eq!(
            batch.get_column("codes").unwrap(),
            &vec![Some("a|b".into()); 2]
        );
    }

    #[test]
    fn test_copy_columns_keeps_positions() {
        let mut batch = make_batch_with_columns(HashMap::from([
            ("a".into(), vec![Some("1".into()), None, None]),
            ("b".into(), vec![Some("2".into()), Some("3".into()), None]),
        ]));
        let params = HashMap::from([
            ("columns".into(), json!(["a", "b"])),
            ("target_column".into(), json!("key")),
            ("value_separator".into(), json!("|")),
        ]);

        copy_columns(&mut batch, &params).unwrap();
        assert_eq!(
            batch.get_column("key").unwrap(),
            &vec![Some("1|2".into()), Some("|3".into()), None]
        );
    }

    #[test]
    fn test_remove_whitespace_from_column_names() {
        let mut batch = make_batch_with_columns(HashMap::from([(
            "  col1 ".into(),
            vec![Some("value".into())],
        )]));

        remove_whitespace_from_columns(&mut batch, &HashMap::new()).unwrap();
        assert_eq!(
            batch.get_column("col1").unwrap(),
            &vec![Some("value".into())]
        );
        assert!(batch.get_column("  col1 ").is_none());
    }

    #[test]
    fn test_copy_columns() {
        let mut batch = make_batch_with_columns(HashMap::from([
            ("col1".into(), vec![Some("a".into()), Some("b".into())]),
            ("col2".into(), vec![Some("1".into()), Some("2".into())]),
        ]));
        let mut params = HashMap::new();
        params.insert("columns".into(), json!(["col1", "col2"]));
        params.insert("target_column".into(), json!("combined"));
        params.insert("value_separator".into(), json!("-"));

        copy_columns(&mut batch, &params).unwrap();
        assert_eq!(
            batch.get_column("combined").unwrap(),
            &vec![Some("a-1".into()), Some("b-2".into())]
        );
    }

    #[test]
    fn test_execute_task_chain() {
        let mut batch = make_batch_with_columns(HashMap::new());
        batch.row_count = 2;
        let registry = TaskRegistry::new();

        let tasks = vec![
            Task {
                task: "add_row_num".into(),
                params: HashMap::from([("starting_index".into(), json!(1))]),
            },
            Task {
                task: "add_constant".into(),
                params: HashMap::from([
                    ("name".into(), json!("const_col")),
                    ("value".into(), json!("const_val")),
                ]),
            },
        ];

        let errors = execute_task_chain(&mut batch, &tasks, &registry);
        assert!(errors.is_empty());
        assert_eq!(
            batch.get_column("rowNum").unwrap(),
            &vec![Some("1".into()), Some("2".into())]
        );
        assert_eq!(
            batch.get_column("const_col").unwrap(),
            &vec![Some("const_val".into()), Some("const_val".into())]
        );
    }

    #[test]
    fn test_execute_task_chain_error_isolation() {
        let mut batch = make_batch_with_columns(HashMap::new());
        batch.row_count = 2;
        let registry = TaskRegistry::new();

        let tasks = vec![
            Task {
                task: "add_constant".into(),
                params: HashMap::from([
                    ("name".into(), json!("good_col")),
                    ("value".into(), json!("good_val")),
                ]),
            },
            Task {
                task: "unknown_task".into(),
                params: HashMap::new(),
            },
            Task {
                task: "add_constant".into(),
                params: HashMap::from([
                    ("name".into(), json!("another_col")),
                    ("value".into(), json!("another_val")),
                ]),
            },
        ];

        let errors = execute_task_chain(&mut batch, &tasks, &registry);
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], Error::UnknownTask(_)));
        assert_eq!(
            batch.get_column("good_col").unwrap(),
            &vec![Some("good_val".into()), Some("good_val".into())]
        );
        assert_eq!(
            batch.get_column("another_col").unwrap(),
            &vec![Some("another_val".into()), Some("another_val".into())]
        );
    }

    #[test]
    fn test_execute_task_chain_missing_param() {
        let mut batch = make_batch_with_columns(HashMap::new());
        batch.row_count = 2;
        let registry = TaskRegistry::new();

        let tasks = vec![Task {
            task: "add_constant".into(),
            params: HashMap::from([("name".into(), json!("col"))]),
        }];

        let errors = execute_task_chain(&mut batch, &tasks, &registry);
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], Error::MissingTaskParam(_)));
    }
}
