use serde::{Deserialize, Serialize};
use std::{fmt, io};

pub type Result<T> = std::result::Result<T, ConverterError>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceLocation {
    pub source: String,
    pub line: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConverterError {
    pub code: String,
    pub message: String,
    pub source_location: Option<SourceLocation>,
}

impl ConverterError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            source_location: None,
        }
    }

    pub fn at(mut self, source: &str, line: u64) -> Self {
        self.source_location = Some(SourceLocation {
            source: source.into(),
            line,
        });
        self
    }
}

impl fmt::Display for ConverterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ConverterError {}

impl From<io::Error> for ConverterError {
    fn from(value: io::Error) -> Self {
        Self::new("io_error", format!("Ошибка чтения или записи: {value}"))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub source_location: Option<SourceLocation>,
}
