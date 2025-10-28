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

impl Error {
    /// Stable numeric code, grouped by pipeline phase: 11xx contract and
    /// configuration, 12xx input reading, 13xx conversion, 14xx serialization.
    /// Codes never change meaning; the message text may.
    pub fn code(&self) -> u16 {
        match self {
            Error::InvalidTimezone(_) => 1101,
            Error::InvalidStreamType(_) => 1102,
            Error::UnknownResourceType(_) => 1103,
            Error::FixedWidthRequiresDictHeaders => 1104,
            Error::UnknownTask(_) => 1105,
            Error::UnknownTaskParam(_) => 1106,
            Error::MissingTaskParam(_) => 1107,
            Error::Config(_) => 1108,
            Error::FileDefinitionNotFound(_) => 1109,
            Error::Csv(_) => 1201,
            Error::Io(_) => 1202,
            Error::Conversion(_) => 1301,
            Error::Json(_) => 1401,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn every_variant_carries_a_phase_numbered_code() {
        assert_eq!(Error::InvalidTimezone("x".into()).code(), 1101);
        assert_eq!(Error::Config("x".into()).code(), 1108);
        assert_eq!(Error::Conversion("x".into()).code(), 1301);
        let io = Error::Io(std::io::Error::other("x"));
        assert_eq!(io.code(), 1202);
    }
}
