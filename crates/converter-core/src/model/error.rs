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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_key: Option<Box<str>>,
    #[serde(default, skip_serializing_if = "message_args_empty")]
    pub message_args: Box<std::collections::BTreeMap<String, serde_json::Value>>,
    pub code: String,
    pub message: String,
    pub source_location: Option<SourceLocation>,
}

impl ConverterError {
    pub fn with_message(mut self, key: &str, args: serde_json::Value) -> Self {
        self.message_key = Some(key.into());
        self.message_args = serde_json::from_value(args).unwrap_or_default();
        self
    }

    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            message_key: None,
            message_args: Default::default(),
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
        Self::new("io_error", format!("Ошибка чтения или записи: {value}")).with_message(
            "messages.io_error",
            crate::model::message_args([("value", value.to_string())]),
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_key: Option<Box<str>>,
    #[serde(default, skip_serializing_if = "message_args_empty")]
    pub message_args: Box<std::collections::BTreeMap<String, serde_json::Value>>,
    pub code: String,
    pub message: String,
    pub source_location: Option<SourceLocation>,
    #[serde(default)]
    pub severity: DiagnosticSeverity,
    #[serde(default = "single_occurrence")]
    pub occurrences: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Info,
    #[default]
    Warning,
}

fn single_occurrence() -> u64 {
    1
}

// Keep argument construction out of recursive DSL parser/renderer stack frames.
#[inline(never)]
pub fn message_args<const N: usize>(pairs: [(&str, String); N]) -> serde_json::Value {
    serde_json::Value::Object(
        pairs
            .into_iter()
            .map(|(key, value)| (key.to_owned(), serde_json::Value::String(value)))
            .collect(),
    )
}

fn message_args_empty(args: &std::collections::BTreeMap<String, serde_json::Value>) -> bool {
    args.is_empty()
}
