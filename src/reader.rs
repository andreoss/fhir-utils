use crate::contract::{FileDefinition, FileType, HeaderDict, Headers, SkipRows};
use crate::error::Error;
use crate::streaming::ChunkedReader;
use csv::ReaderBuilder;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};

#[derive(Debug, Clone)]
pub struct ReaderParams {
    pub file_type: FileType,
    pub value_delimiter: char,
    pub skiprows: Option<SkipRows>,
    pub headers: Option<Headers>,
    pub empty_field_values: Option<Vec<String>>,
}

impl ReaderParams {
    pub fn from_file_definition(
        def: &FileDefinition,
        general_empty_field_values: Option<&Vec<String>>,
    ) -> Self {
        Self {
            file_type: def.file_type.clone(),
            value_delimiter: def.value_delimiter,

            skiprows: def.skiprows.clone(),
            headers: def.headers.clone(),
            empty_field_values: general_empty_field_values.cloned(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RecordBatch {
    pub columns: HashMap<String, Vec<Option<String>>>,
    pub row_count: usize,
}

impl Default for RecordBatch {
    fn default() -> Self {
        Self::new()
    }
}

impl RecordBatch {
    pub fn new() -> Self {
        Self {
            columns: HashMap::new(),
            row_count: 0,
        }
    }

    pub fn add_row(&mut self, row: HashMap<String, Option<String>>) {
        for (col, val) in row {
            self.columns.entry(col).or_default().push(val);
        }
        self.row_count += 1;
    }

    pub fn get_column(&self, name: &str) -> Option<&Vec<Option<String>>> {
        self.columns.get(name)
    }

    pub fn column_names(&self) -> Vec<String> {
        self.columns.keys().cloned().collect()
    }
}

pub fn read_delimited<R: Read + Seek>(
    reader: &mut R,
    params: &ReaderParams,
) -> Result<RecordBatch, Error> {
    let mut params = params.clone();
    params.file_type = FileType::Csv;
    read_all(reader, &params)
}

pub fn read_fixed_width<R: Read + Seek>(
    reader: &mut R,
    params: &ReaderParams,
) -> Result<RecordBatch, Error> {
    let mut params = params.clone();
    params.file_type = FileType::FixedWidth;
    read_all(reader, &params)
}

fn read_all<R: Read + Seek>(reader: &mut R, params: &ReaderParams) -> Result<RecordBatch, Error> {
    let mut chunked = ChunkedReader::new(reader, params.clone(), usize::MAX)?;
    match chunked.next_chunk()? {
        Some(chunk) => Ok(chunk.batch),
        None => Ok(RecordBatch::new()),
    }
}

pub(crate) fn resolve_skip_rows(skiprows: &Option<SkipRows>) -> Result<Vec<usize>, Error> {
    match skiprows {
        Some(SkipRows::Single(n)) => Ok(vec![*n]),
        Some(SkipRows::Multiple(v)) => Ok(v.clone()),
        None => Ok(vec![]),
    }
}

pub(crate) fn resolve_headers<R: Read + Seek>(
    reader: &mut R,
    params: &ReaderParams,
    skip_rows: &[usize],
) -> Result<(Vec<String>, Option<usize>), Error> {
    match &params.headers {
        Some(Headers::List(list)) => Ok((list.clone(), None)),
        Some(Headers::Dict(dict)) => Ok((dict.iter().map(|h| h.name.clone()).collect(), None)),
        Some(Headers::WidthMap(map)) => Ok((
            map.entries().iter().map(|(name, _)| name.clone()).collect(),
            None,
        )),
        None => {
            let mut csv_reader = ReaderBuilder::new()
                .delimiter(params.value_delimiter as u8)
                .has_headers(false)
                .flexible(true)
                .from_reader(&mut *reader);

            let mut header = None;
            for (index, record) in csv_reader.records().enumerate() {
                if skip_rows.contains(&index) {
                    continue;
                }
                let record = record?;
                header = Some((
                    record.iter().map(|value| value.to_string()).collect(),
                    Some(index),
                ));
                break;
            }
            reader.seek(SeekFrom::Start(0))?;
            Ok(header.unwrap_or_else(|| (Vec::new(), None)))
        }
    }
}

pub(crate) fn resolve_fixed_width_headers(
    headers: &Option<Headers>,
) -> Result<Vec<HeaderDict>, Error> {
    match headers {
        Some(Headers::Dict(dict)) => Ok(dict.clone()),
        Some(Headers::WidthMap(map)) => Ok(map
            .entries()
            .iter()
            .map(|(name, width)| HeaderDict {
                name: name.clone(),
                width: *width,
            })
            .collect()),
        _ => Err(Error::FixedWidthRequiresDictHeaders),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{FileDefinition, FileType, General, HeaderDict, Headers, SkipRows};
    use std::io::Cursor;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_general() -> General {
        General {
            time_zone: "America/New_York".into(),
            tenant_id: "test".into(),
            stream_type: "live".into(),
            assigning_authority: None,
            empty_field_values: Some(vec!["".into(), "NA".into(), "N/A".into(), "null".into()]),
            regex_filenames: false,
            extra: Default::default(),
        }
    }

    fn make_csv_def() -> FileDefinition {
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
        }
    }

    fn make_fixed_width_def() -> FileDefinition {
        FileDefinition {
            file_type: FileType::FixedWidth,
            value_delimiter: ',',
            convert_columns_to_string: true,
            resource_type: "Encounter".into(),
            group_by_key: Some("encounterNumber".into()),
            skiprows: None,
            headers: Some(Headers::Dict(vec![
                HeaderDict {
                    name: "col1".into(),
                    width: 10,
                },
                HeaderDict {
                    name: "col2".into(),
                    width: 20,
                },
            ])),
            tasks: None,
            comment: None,
        }
    }

    #[test]
    fn test_read_delimited_basic() {
        let data = "a,b,c\n1,2,3\n4,5,6\n";
        let mut cursor = Cursor::new(data);
        let general = make_general();
        let def = make_csv_def();
        let params = ReaderParams::from_file_definition(&def, general.empty_field_values.as_ref());

        let batch = read_delimited(&mut cursor, &params).unwrap();
        assert_eq!(batch.row_count, 2);
        assert_eq!(batch.column_names().len(), 3);
    }

    #[test]
    fn test_read_delimited_with_headers_list() {
        let data = "1,2,3\n4,5,6\n";
        let mut cursor = Cursor::new(data);
        let general = make_general();
        let mut def = make_csv_def();
        def.headers = Some(Headers::List(vec![
            "col1".into(),
            "col2".into(),
            "col3".into(),
        ]));
        let params = ReaderParams::from_file_definition(&def, general.empty_field_values.as_ref());

        let batch = read_delimited(&mut cursor, &params).unwrap();
        assert_eq!(batch.row_count, 2);
        assert_eq!(
            batch.get_column("col1").unwrap(),
            &vec![Some("1".into()), Some("4".into())]
        );
    }

    #[test]
    fn test_read_delimited_with_skiprows_single() {
        let data = "header\n1,2,3\n4,5,6\n";
        let mut cursor = Cursor::new(data);
        let general = make_general();
        let mut def = make_csv_def();
        def.skiprows = Some(SkipRows::Single(0));
        def.headers = Some(Headers::List(vec![
            "col1".into(),
            "col2".into(),
            "col3".into(),
        ]));
        let params = ReaderParams::from_file_definition(&def, general.empty_field_values.as_ref());

        let batch = read_delimited(&mut cursor, &params).unwrap();
        assert_eq!(batch.row_count, 2);
    }

    #[test]
    fn test_read_delimited_with_skiprows_multiple() {
        let data = "h1\nh2\n1,2,3\n4,5,6\n";
        let mut cursor = Cursor::new(data);
        let general = make_general();
        let mut def = make_csv_def();
        def.skiprows = Some(SkipRows::Multiple(vec![0, 1]));
        def.headers = Some(Headers::List(vec![
            "col1".into(),
            "col2".into(),
            "col3".into(),
        ]));
        let params = ReaderParams::from_file_definition(&def, general.empty_field_values.as_ref());

        let batch = read_delimited(&mut cursor, &params).unwrap();
        assert_eq!(batch.row_count, 2);
    }

    #[test]
    fn test_read_delimited_empty_field_values() {
        let data = "col1,col2,col3\n1,,3\nNA,5,6\n";
        let mut cursor = Cursor::new(data);
        let general = make_general();
        let def = make_csv_def();
        let params = ReaderParams::from_file_definition(&def, general.empty_field_values.as_ref());

        let batch = read_delimited(&mut cursor, &params).unwrap();
        assert_eq!(
            batch.get_column("col2").unwrap(),
            &vec![None, Some("5".into())]
        );
        assert_eq!(
            batch.get_column("col1").unwrap(),
            &vec![Some("1".into()), None]
        );
    }

    #[test]
    fn test_values_are_read_as_strings_whatever_the_contract_asks() {
        let data = "1,2.5,true\n4,5,6\n";
        let mut cursor = Cursor::new(data);
        let general = make_general();
        let mut def = make_csv_def();
        def.convert_columns_to_string = false;
        def.headers = Some(Headers::List(vec![
            "col1".into(),
            "col2".into(),
            "col3".into(),
        ]));
        let params = ReaderParams::from_file_definition(&def, general.empty_field_values.as_ref());

        let batch = read_delimited(&mut cursor, &params).unwrap();
        assert_eq!(
            batch.get_column("col2").unwrap(),
            &vec![Some("2.5".into()), Some("5".into())]
        );
        assert_eq!(
            batch.get_column("col3").unwrap(),
            &vec![Some("true".into()), Some("6".into())]
        );
        assert_eq!(
            batch.get_column("col1").unwrap(),
            &vec![Some("1".into()), Some("4".into())]
        );
    }

    #[test]
    fn test_read_fixed_width_basic() {
        let data = "1234567890abcdefghij\n0987654321klmnopqrst\n";
        let mut cursor = Cursor::new(data);
        let general = make_general();
        let def = make_fixed_width_def();
        let params = ReaderParams::from_file_definition(&def, general.empty_field_values.as_ref());

        let batch = read_fixed_width(&mut cursor, &params).unwrap();
        assert_eq!(batch.row_count, 2);
        assert_eq!(
            batch.get_column("col1").unwrap(),
            &vec![Some("1234567890".into()), Some("0987654321".into())]
        );
        assert_eq!(
            batch.get_column("col2").unwrap(),
            &vec![Some("abcdefghij".into()), Some("klmnopqrst".into())]
        );
    }

    #[test]
    fn test_read_fixed_width_with_skiprows() {
        let data = "header\n1234567890abcdefghij\n0987654321klmnopqrst\n";
        let mut cursor = Cursor::new(data);
        let general = make_general();
        let mut def = make_fixed_width_def();
        def.skiprows = Some(SkipRows::Single(0));
        let params = ReaderParams::from_file_definition(&def, general.empty_field_values.as_ref());

        let batch = read_fixed_width(&mut cursor, &params).unwrap();
        assert_eq!(batch.row_count, 2);
    }

    #[test]
    fn test_read_fixed_width_empty_field_values() {
        let data = "1234567890          \n          abcdefghij\n";
        let mut cursor = Cursor::new(data);
        let general = make_general();
        let def = make_fixed_width_def();
        let params = ReaderParams::from_file_definition(&def, general.empty_field_values.as_ref());

        let batch = read_fixed_width(&mut cursor, &params).unwrap();
        assert_eq!(
            batch.get_column("col2").unwrap(),
            &vec![None, Some("abcdefghij".into())]
        );
        assert_eq!(
            batch.get_column("col1").unwrap(),
            &vec![Some("1234567890".into()), None]
        );
    }

    #[test]
    fn test_read_fixed_width_width_map_headers() {
        let data = "1234567890abcdefghij\n";
        let mut cursor = Cursor::new(data);
        let general = make_general();
        let mut def = make_fixed_width_def();
        def.headers = Some(Headers::WidthMap(crate::contract::WidthMap(vec![
            ("field1".into(), 10),
            ("field2".into(), 20),
        ])));
        let params = ReaderParams::from_file_definition(&def, general.empty_field_values.as_ref());

        let batch = read_fixed_width(&mut cursor, &params).unwrap();
        assert_eq!(batch.row_count, 1);
        assert_eq!(
            batch.get_column("field1").unwrap(),
            &vec![Some("1234567890".into())]
        );
        assert_eq!(
            batch.get_column("field2").unwrap(),
            &vec![Some("abcdefghij".into())]
        );
    }

    #[test]
    fn test_width_map_headers_keep_contract_order() {
        let contract = r#"{
            "general": {"timeZone": "UTC", "tenantId": "t1", "streamType": "live"},
            "fileDefinitions": {
                "obs": {
                    "fileType": "fixed-width",
                    "resourceType": "Observation",
                    "groupByKey": "patientInternalId",
                    "headers": {"patientInternalId": 4, "observationCode": 6, "observationValue": 3}
                }
            }
        }"#;
        let contract = crate::contract::Contract::load(contract).unwrap();
        let def = contract.file_definitions.get("obs").unwrap();
        let params = ReaderParams::from_file_definition(def, None);

        let mut cursor = Cursor::new("p1  1234-5 42\n");
        let batch = read_fixed_width(&mut cursor, &params).unwrap();
        assert_eq!(
            batch.get_column("patientInternalId").unwrap(),
            &vec![Some("p1".into())]
        );
        assert_eq!(
            batch.get_column("observationCode").unwrap(),
            &vec![Some("1234-5".into())]
        );
        assert_eq!(
            batch.get_column("observationValue").unwrap(),
            &vec![Some("42".into())]
        );
    }

    #[test]
    fn test_read_file_csv() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "col1,col2,col3").unwrap();
        writeln!(file, "1,2,3").unwrap();
        writeln!(file, "4,5,6").unwrap();
        file.flush().unwrap();

        let general = make_general();
        let def = make_csv_def();
        let params = ReaderParams::from_file_definition(&def, general.empty_field_values.as_ref());

        let mut handle = std::fs::File::open(file.path()).unwrap();
        let batch = read_delimited(&mut handle, &params).unwrap();
        assert_eq!(batch.row_count, 2);
    }

    #[test]
    fn test_read_file_fixed_width() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "1234567890abcdefghij").unwrap();
        writeln!(file, "0987654321klmnopqrst").unwrap();
        file.flush().unwrap();

        let general = make_general();
        let def = make_fixed_width_def();
        let params = ReaderParams::from_file_definition(&def, general.empty_field_values.as_ref());
        let mut handle = std::fs::File::open(file.path()).unwrap();
        let batch = read_fixed_width(&mut handle, &params).unwrap();
        assert_eq!(batch.row_count, 2);
    }

    #[test]
    fn test_fixed_width_requires_dict_headers() {
        let data = "1234567890abcdefghij\n";
        let mut cursor = Cursor::new(data);
        let general = make_general();
        let mut def = make_fixed_width_def();
        def.headers = Some(Headers::List(vec!["col1".into(), "col2".into()]));
        let params = ReaderParams::from_file_definition(&def, general.empty_field_values.as_ref());

        let err = read_fixed_width(&mut cursor, &params).unwrap_err();
        assert!(matches!(err, Error::FixedWidthRequiresDictHeaders));
    }
}
