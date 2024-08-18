use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid timezone: {0}")]
    InvalidTimezone(String),
    #[error("invalid stream type: {0}")]
    InvalidStreamType(String),
    #[error("unknown resource type: {0}")]
    UnknownResourceType(String),
    #[error("fixed-width requires dict headers")]
    FixedWidthRequiresDictHeaders,
    #[error("unknown task: {0}")]
    UnknownTask(String),
    #[error("unknown task parameter: {0}")]
    UnknownTaskParam(String),
    #[error("missing task parameter: {0}")]
    MissingTaskParam(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),
    #[error("config error: {0}")]
    Config(String),
    #[error("conversion error: {0}")]
    Conversion(String),
    #[error("file definition not found for: {0}")]
    FileDefinitionNotFound(String),
}
