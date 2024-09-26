use crate::contract::{FileType, HeaderDict};
use crate::error::Error;
use crate::reader::{
    resolve_fixed_width_headers, resolve_headers, resolve_skip_rows, ReaderParams, RecordBatch,
};
use csv::{Reader, ReaderBuilder};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

pub struct Chunk {
    pub batch: RecordBatch,
    pub starting_row_num: usize,
}

enum Source<R: Read + Seek> {
    Delimited(Box<Reader<R>>),
    FixedWidth(BufReader<R>),
}

pub struct ChunkedReader<R: Read + Seek> {
    source: Source<R>,
    params: ReaderParams,
    buffer_size: usize,
    headers: Vec<String>,
    widths: Vec<HeaderDict>,
    skip_rows: Vec<usize>,
    has_explicit_headers: bool,
    row_index: usize,
    next_row_num: usize,
    finished: bool,
}

impl<R: Read + Seek> ChunkedReader<R> {
    pub fn new(mut reader: R, params: ReaderParams, buffer_size: usize) -> Result<Self, Error> {
        let skip_rows = resolve_skip_rows(&params.skiprows)?;
        let has_explicit_headers = params.headers.is_some();
        let buffer_size = if buffer_size == 0 {
            usize::MAX
        } else {
            buffer_size
        };

        let (source, headers, widths) = match params.file_type {
            FileType::Csv => {
                let headers = resolve_headers(&mut reader, &params)?;
                reader.seek(SeekFrom::Start(0))?;
                let csv_reader = ReaderBuilder::new()
                    .delimiter(params.value_delimiter as u8)
                    .has_headers(false)
                    .flexible(true)
                    .from_reader(reader);
                (Source::Delimited(Box::new(csv_reader)), headers, Vec::new())
            }
            FileType::FixedWidth => {
                let widths = resolve_fixed_width_headers(&params.headers)?;
                reader.seek(SeekFrom::Start(0))?;
                let headers = widths.iter().map(|h| h.name.clone()).collect();
                (Source::FixedWidth(BufReader::new(reader)), headers, widths)
            }
        };

        Ok(Self {
            source,
            params,
            buffer_size,
            headers,
            widths,
            skip_rows,
            has_explicit_headers,
            row_index: 0,
            next_row_num: 1,
            finished: false,
        })
    }

    pub fn next_chunk(&mut self) -> Result<Option<Chunk>, Error> {
        if self.finished {
            return Ok(None);
        }

        let Self {
            source,
            params,
            buffer_size,
            headers,
            widths,
            skip_rows,
            has_explicit_headers,
            row_index,
            next_row_num,
            finished,
        } = self;

        let starting_row_num = *next_row_num;
        let mut batch = RecordBatch::new();
        let mut rows_read = 0usize;

        match source {
            Source::Delimited(reader) => {
                for result in reader.records() {
                    let record = result?;

                    if !*has_explicit_headers && *row_index == 0 {
                        *row_index += 1;
                        continue;
                    }
                    if skip_rows.contains(row_index) {
                        *row_index += 1;
                        continue;
                    }

                    let mut row = HashMap::new();
                    for (index, header) in headers.iter().enumerate() {
                        row.insert(header.clone(), normalize(record.get(index), params));
                    }
                    batch.add_row(row);
                    *row_index += 1;
                    rows_read += 1;

                    if rows_read >= *buffer_size {
                        break;
                    }
                }
            }
            Source::FixedWidth(reader) => {
                let mut line = String::new();
                while reader.read_line(&mut line)? > 0 {
                    let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');

                    if skip_rows.contains(row_index) {
                        *row_index += 1;
                        line.clear();
                        continue;
                    }

                    let mut row = HashMap::new();
                    let mut pos = 0;
                    for header in widths.iter() {
                        let value = if pos < trimmed.len() {
                            let end = (pos + header.width).min(trimmed.len());
                            normalize(Some(trimmed[pos..end].trim_end()), params)
                        } else {
                            None
                        };
                        row.insert(header.name.clone(), value);
                        pos += header.width;
                    }
                    batch.add_row(row);
                    *row_index += 1;
                    rows_read += 1;
                    line.clear();

                    if rows_read >= *buffer_size {
                        break;
                    }
                }
            }
        }

        if rows_read == 0 {
            *finished = true;
            return Ok(None);
        }

        *next_row_num += rows_read;
        Ok(Some(Chunk {
            batch,
            starting_row_num,
        }))
    }
}

impl<R: Read + Seek> Iterator for ChunkedReader<R> {
    type Item = Result<Chunk, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_chunk().transpose()
    }
}

pub fn read_file_chunked(
    path: &Path,
    params: &ReaderParams,
    buffer_size: usize,
) -> Result<ChunkedReader<File>, Error> {
    let file = File::open(path)?;
    ChunkedReader::new(file, params.clone(), buffer_size)
}

fn normalize(value: Option<&str>, params: &ReaderParams) -> Option<String> {
    let value = value?;
    let empty = params
        .empty_field_values
        .as_ref()
        .is_some_and(|values| values.iter().any(|candidate| candidate == value));
    if value.is_empty() || empty {
        None
    } else {
        Some(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use crate::contract::{FileDefinition, FileType, General, HeaderDict, Headers, SkipRows};
    use crate::default_tasks::build_default_task_chain_with_start;
    use crate::reader::ReaderParams;
    use crate::streaming::{read_file_chunked, ChunkedReader};
    use crate::tasks::{execute_task_chain, TaskRegistry};
    use std::io::{Cursor, Write};
    use tempfile::NamedTempFile;

    fn general() -> General {
        General {
            time_zone: "America/New_York".into(),
            tenant_id: "test".into(),
            stream_type: "live".into(),
            assigning_authority: None,
            empty_field_values: Some(vec!["NA".into()]),
            regex_filenames: false,
        }
    }

    fn csv_def() -> FileDefinition {
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

    fn params(def: &FileDefinition, gen: &General) -> ReaderParams {
        ReaderParams::from_file_definition(def, gen.empty_field_values.as_ref())
    }

    #[test]
    fn chunks_split_on_buffer_size() {
        let data = "a,b\n1,2\n3,4\n5,6\n";
        let p = params(&csv_def(), &general());
        let mut reader = ChunkedReader::new(Cursor::new(data), p, 2).unwrap();

        let first = reader.next_chunk().unwrap().unwrap();
        assert_eq!(first.batch.row_count, 2);
        assert_eq!(first.starting_row_num, 1);

        let second = reader.next_chunk().unwrap().unwrap();
        assert_eq!(second.batch.row_count, 1);
        assert_eq!(second.starting_row_num, 3);

        assert!(reader.next_chunk().unwrap().is_none());
    }

    #[test]
    fn row_num_continues_across_chunks() {
        let data = "a,b\n1,2\n3,4\n5,6\n7,8\n9,10\n";
        let def = csv_def();
        let gen = general();
        let registry = TaskRegistry::new();
        let mut reader = ChunkedReader::new(Cursor::new(data), params(&def, &gen), 2).unwrap();

        let mut seen = Vec::new();
        while let Some(mut chunk) = reader.next_chunk().unwrap() {
            let tasks = build_default_task_chain_with_start(
                &gen,
                &def,
                "in.csv",
                &def.resource_type,
                chunk.starting_row_num,
            );
            let errors = execute_task_chain(&mut chunk.batch, &tasks, &registry);
            assert!(errors.is_empty());
            for value in chunk.batch.get_column("rowNum").unwrap() {
                seen.push(value.clone().unwrap());
            }
        }

        assert_eq!(seen, vec!["1", "2", "3", "4", "5"]);
    }

    #[test]
    fn iterator_yields_every_chunk() {
        let data = "a,b\n1,2\n3,4\n5,6\n";
        let p = params(&csv_def(), &general());
        let reader = ChunkedReader::new(Cursor::new(data), p, 2).unwrap();
        let chunks: Vec<_> = reader.map(|c| c.unwrap()).collect();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[1].starting_row_num, 3);
    }

    #[test]
    fn explicit_headers_keep_first_data_row() {
        let data = "1,2\n3,4\n";
        let mut def = csv_def();
        def.headers = Some(Headers::List(vec!["a".into(), "b".into()]));
        let p = params(&def, &general());
        let mut reader = ChunkedReader::new(Cursor::new(data), p, 10).unwrap();

        let chunk = reader.next_chunk().unwrap().unwrap();
        assert_eq!(chunk.batch.row_count, 2);
        assert_eq!(
            chunk.batch.get_column("a").unwrap(),
            &vec![Some("1".into()), Some("3".into())]
        );
    }

    #[test]
    fn skiprows_applies_across_chunks() {
        let data = "x,y\n1,2\n3,4\n5,6\n";
        let mut def = csv_def();
        def.headers = Some(Headers::List(vec!["a".into(), "b".into()]));
        def.skiprows = Some(SkipRows::Multiple(vec![0, 2]));
        let p = params(&def, &general());
        let mut reader = ChunkedReader::new(Cursor::new(data), p, 1).unwrap();

        let first = reader.next_chunk().unwrap().unwrap();
        assert_eq!(
            first.batch.get_column("a").unwrap(),
            &vec![Some("1".into())]
        );
        let second = reader.next_chunk().unwrap().unwrap();
        assert_eq!(
            second.batch.get_column("a").unwrap(),
            &vec![Some("5".into())]
        );
        assert_eq!(second.starting_row_num, 2);
        assert!(reader.next_chunk().unwrap().is_none());
    }

    #[test]
    fn empty_field_values_become_null() {
        let data = "a,b\n1,NA\n,4\n";
        let p = params(&csv_def(), &general());
        let mut reader = ChunkedReader::new(Cursor::new(data), p, 10).unwrap();

        let chunk = reader.next_chunk().unwrap().unwrap();
        assert_eq!(
            chunk.batch.get_column("b").unwrap(),
            &vec![None, Some("4".into())]
        );
        assert_eq!(
            chunk.batch.get_column("a").unwrap(),
            &vec![Some("1".into()), None]
        );
    }

    #[test]
    fn fixed_width_reads_in_chunks() {
        let data = "abc12\ndef34\nghi56\n";
        let mut def = csv_def();
        def.file_type = FileType::FixedWidth;
        def.headers = Some(Headers::Dict(vec![
            HeaderDict {
                name: "a".into(),
                width: 3,
            },
            HeaderDict {
                name: "b".into(),
                width: 2,
            },
        ]));
        let p = params(&def, &general());
        let mut reader = ChunkedReader::new(Cursor::new(data), p, 2).unwrap();

        let first = reader.next_chunk().unwrap().unwrap();
        assert_eq!(first.batch.row_count, 2);
        assert_eq!(
            first.batch.get_column("a").unwrap(),
            &vec![Some("abc".into()), Some("def".into())]
        );
        let second = reader.next_chunk().unwrap().unwrap();
        assert_eq!(second.starting_row_num, 3);
        assert_eq!(
            second.batch.get_column("b").unwrap(),
            &vec![Some("56".into())]
        );
    }

    #[test]
    fn chunks_never_exceed_the_buffer_size() {
        let mut data = String::from("a,b\n");
        for row in 0..5000 {
            data.push_str(&format!("{row},{row}\n"));
        }
        let p = params(&csv_def(), &general());
        let mut reader = ChunkedReader::new(Cursor::new(data), p, 10).unwrap();

        let mut chunks = 0;
        let mut rows = 0;
        let mut next_row_num = 1;
        while let Some(chunk) = reader.next_chunk().unwrap() {
            assert!(chunk.batch.row_count <= 10);
            assert_eq!(chunk.starting_row_num, next_row_num);
            next_row_num += chunk.batch.row_count;
            rows += chunk.batch.row_count;
            chunks += 1;
        }

        assert_eq!(rows, 5000);
        assert_eq!(chunks, 500);
    }

    #[test]
    fn reads_a_file_in_chunks() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "a,b").unwrap();
        writeln!(file, "1,2").unwrap();
        writeln!(file, "3,4").unwrap();
        file.flush().unwrap();

        let def = csv_def();
        let p = params(&def, &general());
        let mut reader = read_file_chunked(file.path(), &p, 1).unwrap();
        assert_eq!(reader.next_chunk().unwrap().unwrap().starting_row_num, 1);
        assert_eq!(reader.next_chunk().unwrap().unwrap().starting_row_num, 2);
        assert!(reader.next_chunk().unwrap().is_none());
    }
}
