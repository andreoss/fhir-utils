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
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::MissingTaskParam("add_constant.value".into()))?;

    let column = batch.columns.entry(name.to_string()).or_default();
    column.resize(batch.row_count, Some(value.to_string()));
    Ok(())
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

fn set_nan_to_none(
    _batch: &mut RecordBatch,
    _params: &HashMap<String, Value>,
) -> Result<(), Error> {
    for column in _batch.columns.values_mut() {
        for value in column.iter_mut() {
            if let Some(v) = value {
                let trimmed = v.trim();
                if trimmed.is_empty()
                    || trimmed.eq_ignore_ascii_case("nan")
                    || trimmed.eq_ignore_ascii_case("na")
                    || trimmed.eq_ignore_ascii_case("n/a")
                    || trimmed.eq_ignore_ascii_case("null")
                {
                    *value = None;
                }
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

    let mut target = Vec::with_capacity(batch.row_count);
    for row_idx in 0..batch.row_count {
        let mut parts = Vec::new();
        for col_name in &source_columns {
            if let Some(col) = batch.columns.get(col_name) {
                if let Some(Some(val)) = col.get(row_idx) {
                    parts.push(val.clone());
                }
            }
        }
        target.push(if parts.is_empty() {
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
                Some("NAN".into()),
                Some("  ".into()),
                Some("null".into()),
            ],
        )]));
        let params = HashMap::new();

        set_nan_to_none(&mut batch, &params).unwrap();
        assert_eq!(
            batch.get_column("col1").unwrap(),
            &vec![Some("value".into()), None, None, None]
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
