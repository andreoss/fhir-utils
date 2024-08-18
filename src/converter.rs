use crate::contract::{Contract, FileDefinition};
use crate::default_tasks::build_default_task_chain_with_start;
use crate::error::Error;
use crate::reader::ReaderParams;
use crate::streaming::ChunkedReader;
use crate::tasks::{execute_task_chain, TaskRegistry};
use serde_json::{Map, Value};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;
use tracing::warn;

pub type RecordConverter = fn(&str, &Value) -> Result<Vec<Value>, Error>;

#[derive(Clone, Copy)]
pub struct ConversionOptions<'a> {
    pub contract: &'a Contract,
    pub file_def: &'a FileDefinition,
    pub file_path: &'a str,
    pub registry: &'a TaskRegistry,
    pub buffer_size: usize,
}

#[derive(Debug)]
pub struct TransformedRow {
    pub exception: Option<Error>,
    pub group_by_key: String,
    pub record: Value,
}

#[derive(Debug)]
pub struct ConvertedRow {
    pub exception: Option<Error>,
    pub group_by_key: String,
    pub resources: Vec<Value>,
}

pub struct Transform<'a, R: Read + Seek> {
    reader: ChunkedReader<R>,
    options: ConversionOptions<'a>,
    pending: VecDeque<TransformedRow>,
    done: bool,
}

impl<'a, R: Read + Seek> Transform<'a, R> {
    pub fn from_reader(reader: R, options: ConversionOptions<'a>) -> Result<Self, Error> {
        let params = ReaderParams::from_file_definition(
            options.file_def,
            options.contract.general.empty_field_values.as_ref(),
        );
        let reader = ChunkedReader::new(reader, params, options.buffer_size)?;
        Ok(Self {
            reader,
            options,
            pending: VecDeque::new(),
            done: false,
        })
    }

    fn fill(&mut self) -> Result<bool, Error> {
        let chunk = match self.reader.next_chunk()? {
            Some(chunk) => chunk,
            None => return Ok(false),
        };
        let mut batch = chunk.batch;

        let mut tasks = build_default_task_chain_with_start(
            &self.options.contract.general,
            self.options.file_def,
            self.options.file_path,
            &self.options.file_def.resource_type,
            chunk.starting_row_num,
        );
        if let Some(user_tasks) = &self.options.file_def.tasks {
            tasks.extend(user_tasks.clone());
        }
        for error in execute_task_chain(&mut batch, &tasks, self.options.registry) {
            warn!(error = %error, "task failed");
        }

        for index in 0..batch.row_count {
            let mut record = Map::new();
            for (name, values) in &batch.columns {
                let value = match values.get(index) {
                    Some(Some(value)) => Value::String(value.clone()),
                    _ => Value::Null,
                };
                record.insert(name.clone(), value);
            }
            let group_by_key = record
                .get("groupByKey")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();
            self.pending.push_back(TransformedRow {
                exception: None,
                group_by_key,
                record: Value::Object(record),
            });
        }

        Ok(true)
    }
}

impl<R: Read + Seek> Iterator for Transform<'_, R> {
    type Item = TransformedRow;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(row) = self.pending.pop_front() {
                return Some(row);
            }
            if self.done {
                return None;
            }
            match self.fill() {
                Ok(true) => continue,
                Ok(false) => {
                    self.done = true;
                    return None;
                }
                Err(error) => {
                    self.done = true;
                    return Some(TransformedRow {
                        exception: Some(error),
                        group_by_key: String::new(),
                        record: Value::Null,
                    });
                }
            }
        }
    }
}

pub struct Convert<'a, R: Read + Seek> {
    transform: Transform<'a, R>,
    converter: RecordConverter,
}

impl<R: Read + Seek> Iterator for Convert<'_, R> {
    type Item = ConvertedRow;

    fn next(&mut self) -> Option<Self::Item> {
        let row = self.transform.next()?;
        let resource_type = self.transform.options.file_def.resource_type.clone();

        if let Some(error) = row.exception {
            return Some(ConvertedRow {
                exception: Some(error),
                group_by_key: row.group_by_key,
                resources: Vec::new(),
            });
        }

        Some(match (self.converter)(&resource_type, &row.record) {
            Ok(resources) => ConvertedRow {
                exception: None,
                group_by_key: row.group_by_key,
                resources,
            },
            Err(error) => ConvertedRow {
                exception: Some(error),
                group_by_key: row.group_by_key,
                resources: Vec::new(),
            },
        })
    }
}

pub fn transform<'a>(
    path: &Path,
    options: ConversionOptions<'a>,
) -> Result<Transform<'a, File>, Error> {
    Transform::from_reader(File::open(path)?, options)
}

pub fn convert<'a>(
    path: &Path,
    options: ConversionOptions<'a>,
) -> Result<Convert<'a, File>, Error> {
    convert_with(File::open(path)?, options, passthrough)
}

pub fn convert_with<'a, R: Read + Seek>(
    reader: R,
    options: ConversionOptions<'a>,
    converter: RecordConverter,
) -> Result<Convert<'a, R>, Error> {
    Ok(Convert {
        transform: Transform::from_reader(reader, options)?,
        converter,
    })
}

fn passthrough(_resource_type: &str, record: &Value) -> Result<Vec<Value>, Error> {
    Ok(vec![record.clone()])
}

#[cfg(test)]
mod tests {
    use crate::contract::{Contract, FileDefinition, FileType, General, Task};
    use crate::converter::{convert, convert_with, transform, ConversionOptions, Transform};
    use crate::error::Error;
    use crate::tasks::TaskRegistry;
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::io::{Cursor, Write};
    use tempfile::NamedTempFile;

    fn general() -> General {
        General {
            time_zone: "America/New_York".into(),
            tenant_id: "test".into(),
            stream_type: "live".into(),
            assigning_authority: None,
            empty_field_values: None,
            regex_filenames: false,
        }
    }

    fn file_def() -> FileDefinition {
        FileDefinition {
            file_type: FileType::Csv,
            value_delimiter: ',',
            convert_columns_to_string: true,
            resource_type: "Patient".into(),
            group_by_key: Some("a".into()),
            skiprows: None,
            headers: None,
            tasks: None,
            comment: None,
        }
    }

    fn contract(def: &FileDefinition) -> Contract {
        Contract {
            general: general(),
            file_definitions: HashMap::from([("in.csv".to_string(), def.clone())]),
        }
    }

    fn options<'a>(
        contract: &'a Contract,
        def: &'a FileDefinition,
        registry: &'a TaskRegistry,
        buffer_size: usize,
    ) -> ConversionOptions<'a> {
        ConversionOptions {
            contract,
            file_def: def,
            file_path: "in.csv",
            registry,
            buffer_size,
        }
    }

    #[test]
    fn transform_applies_default_tasks_per_row() {
        let def = file_def();
        let contract = contract(&def);
        let registry = TaskRegistry::new();
        let data = "a,b\n1,2\n3,4\n";
        let rows: Vec<_> =
            Transform::from_reader(Cursor::new(data), options(&contract, &def, &registry, 1))
                .unwrap()
                .collect();

        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.exception.is_none()));
        assert_eq!(rows[0].record["rowNum"], json!("1"));
        assert_eq!(rows[1].record["rowNum"], json!("2"));
        assert_eq!(rows[0].record["tenantId"], json!("test"));
        assert_eq!(rows[0].record["filePath"], json!("in.csv"));
        assert_eq!(rows[0].record["configResourceType"], json!("Patient"));
        assert_eq!(rows[0].group_by_key, "1");
        assert_eq!(rows[1].group_by_key, "3");
    }

    #[test]
    fn transform_streams_without_reading_everything() {
        let def = file_def();
        let contract = contract(&def);
        let registry = TaskRegistry::new();
        let data = "a,b\n1,2\n3,4\n5,6\n";
        let rows: Vec<_> =
            Transform::from_reader(Cursor::new(data), options(&contract, &def, &registry, 1))
                .unwrap()
                .take(2)
                .collect();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].record["rowNum"], json!("2"));
    }

    #[test]
    fn empty_input_yields_no_rows() {
        let def = file_def();
        let contract = contract(&def);
        let registry = TaskRegistry::new();
        let rows: Vec<_> = Transform::from_reader(
            Cursor::new("a,b\n"),
            options(&contract, &def, &registry, 10),
        )
        .unwrap()
        .collect();

        assert!(rows.is_empty());
    }

    #[test]
    fn failing_task_does_not_stop_the_pipeline() {
        let mut def = file_def();
        def.tasks = Some(vec![Task {
            task: "add_constant".into(),
            params: HashMap::new(),
        }]);
        let contract = contract(&def);
        let registry = TaskRegistry::new();
        let rows: Vec<_> = Transform::from_reader(
            Cursor::new("a,b\n1,2\n3,4\n"),
            options(&contract, &def, &registry, 10),
        )
        .unwrap()
        .collect();

        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.exception.is_none()));
    }

    #[test]
    fn convert_yields_group_key_and_resources() {
        let def = file_def();
        let contract = contract(&def);
        let registry = TaskRegistry::new();
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "a,b").unwrap();
        writeln!(file, "1,2").unwrap();
        file.flush().unwrap();

        let rows: Vec<_> = convert(file.path(), options(&contract, &def, &registry, 10))
            .unwrap()
            .collect();

        assert_eq!(rows.len(), 1);
        assert!(rows[0].exception.is_none());
        assert_eq!(rows[0].group_by_key, "1");
        assert_eq!(rows[0].resources.len(), 1);
        assert_eq!(rows[0].resources[0]["a"], json!("1"));
    }

    #[test]
    fn convert_yields_row_errors_and_keeps_going() {
        fn failing(_resource_type: &str, record: &Value) -> Result<Vec<Value>, Error> {
            if record["a"] == json!("3") {
                Err(Error::Conversion("row rejected".into()))
            } else {
                Ok(vec![record.clone()])
            }
        }

        let def = file_def();
        let contract = contract(&def);
        let registry = TaskRegistry::new();
        let rows: Vec<_> = convert_with(
            Cursor::new("a,b\n1,2\n3,4\n5,6\n"),
            options(&contract, &def, &registry, 2),
            failing,
        )
        .unwrap()
        .collect();

        assert_eq!(rows.len(), 3);
        assert!(rows[0].exception.is_none());
        assert!(rows[1].exception.is_some());
        assert!(rows[1].resources.is_empty());
        assert_eq!(rows[1].group_by_key, "3");
        assert!(rows[2].exception.is_none());
    }

    #[test]
    fn transform_reads_from_a_path() {
        let def = file_def();
        let contract = contract(&def);
        let registry = TaskRegistry::new();
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "a,b").unwrap();
        writeln!(file, "7,8").unwrap();
        file.flush().unwrap();

        let rows: Vec<_> = transform(file.path(), options(&contract, &def, &registry, 10))
            .unwrap()
            .collect();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].group_by_key, "7");
    }
}
