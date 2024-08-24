mod config;
mod contract;
mod converter;
mod default_tasks;
mod error;
pub mod fhirrs;
pub mod fhirutils;
mod integration_tests;
mod lookup;
mod reader;
mod streaming;
mod tasks;

pub use config::Config;
pub use contract::{Contract, FileDefinition, General};
pub use converter::{
    convert, convert_with, transform, ConversionOptions, ConvertedRow, RecordConverter,
    TransformedRow,
};
pub use default_tasks::{
    build_default_task_chain, build_default_task_chain_with_start, execute_default_and_user_tasks,
};
pub use error::Error;
pub use lookup::lookup_file_definition;
pub use reader::{read_file, ReaderParams, RecordBatch};
pub use streaming::{read_file_chunked, Chunk, ChunkedReader};
pub use tasks::{execute_task_chain, TaskRegistry};
