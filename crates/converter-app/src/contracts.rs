use converter_core::model::{
    ConversionSummary, Diagnostic, DictionaryMetadata, ReadOptions, StageProgress,
};
use converter_download::DownloadProgress;
use serde::{Deserialize, Serialize};
use std::{fmt, path::PathBuf};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppError {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_key: Option<Box<str>>,
    #[serde(default, skip_serializing_if = "message_args_empty")]
    pub message_args: Box<std::collections::BTreeMap<String, serde_json::Value>>,
    pub code: String,
    pub message: String,
}
impl AppError {
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
        }
    }
}
impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for AppError {}
impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::new("storage_error", format!("Ошибка хранилища: {:?}", e.kind())).with_message(
            "messages.download_io",
            converter_core::model::message_args([("arg0", format!("{:?}", e.kind()))]),
        )
    }
}
impl From<converter_core::model::ConverterError> for AppError {
    fn from(e: converter_core::model::ConverterError) -> Self {
        Self {
            code: e.code,
            message: e.message,
            message_key: e.message_key,
            message_args: e.message_args,
        }
    }
}
impl From<converter_download::DownloadError> for AppError {
    fn from(e: converter_download::DownloadError) -> Self {
        Self {
            code: e.code,
            message: e.message,
            message_key: e.message_key,
            message_args: e.message_args,
        }
    }
}
impl From<serde_json::Error> for AppError {
    fn from(_: serde_json::Error) -> Self {
        Self::new("invalid_store", "Не удалось прочитать состояние приложения").with_message(
            "messages.invalid_store",
            converter_core::model::message_args([]),
        )
    }
}
pub type AppResult<T> = Result<T, AppError>;

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceInput {
    Local {
        import_id: String,
    },
    Remote {
        url: String,
        expected_sha256: Option<String>,
    },
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputChoice {
    DesktopFolder(PathBuf),
    #[default]
    AppLibrary,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct JobDraft {
    pub source: SourceInput,
    pub resources: Vec<SourceInput>,
    pub output: OutputChoice,
    pub options: ReadOptions,
    pub metadata_override: Option<DictionaryMetadata>,
    pub auto_start: bool,
    #[serde(default)]
    pub prepare_only: bool,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct JobOptionsUpdate {
    pub options: Option<ReadOptions>,
    pub metadata_override: Option<DictionaryMetadata>,
    pub output: Option<OutputChoice>,
    pub dictionary_id: Option<String>,
    pub resources: Option<Vec<String>>,
    pub resource_sources: Option<Vec<SourceInput>>,
    pub retained_resource_ids: Option<Vec<String>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Ready,
    Downloading,
    DownloadRetryWait,
    DownloadPaused,
    DownloadFailed,
    Importing,
    Inspecting,
    AwaitingInput,
    Converting,
    Validating,
    Publishing,
    Succeeded,
    SucceededWithWarnings,
    Failed,
    Cancelled,
    Interrupted,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadKind {
    Local,
    Remote,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadJobProgress {
    pub kind: DownloadKind,
    pub files: Vec<DownloadProgress>,
    pub received_bytes: u64,
    pub confirmed_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: Option<f64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversionStatus {
    NotStarted,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConversionJobProgress {
    pub status: ConversionStatus,
    pub current: Option<StageProgress>,
    pub completed_stages: Vec<StageProgress>,
}
impl Default for ConversionJobProgress {
    fn default() -> Self {
        Self {
            status: ConversionStatus::NotStarted,
            current: None,
            completed_stages: vec![],
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExportSnapshot {
    pub status: ExportStatus,
    pub saved_files: Vec<String>,
    pub error: Option<AppError>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportStatus {
    #[default]
    NotStarted,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResultFile {
    pub name: String,
    pub bytes: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResultInfo {
    pub id: String,
    pub files: Vec<ResultFile>,
    pub summary: ConversionSummary,
    pub can_open: bool,
    pub can_export: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputChoice {
    pub id: String,
    pub display_name: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdditionalDictionariesOffer {
    pub id: String,
    pub main_name: String,
    pub dictionaries: Vec<InputChoice>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobSnapshot {
    #[serde(default)]
    pub resource_attachments: Vec<ResourceAttachment>,
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub source_bytes: Option<u64>,
    #[serde(default)]
    pub prepare_only: bool,
    #[serde(default)]
    pub had_errors: bool,
    #[serde(default)]
    pub output: OutputChoice,
    pub generation: u64,
    pub run_id: String,
    pub sequence: u64,
    pub state: JobState,
    pub download: DownloadJobProgress,
    pub conversion: ConversionJobProgress,
    pub result: Option<ResultInfo>,
    pub export: ExportSnapshot,
    pub error: Option<AppError>,
    pub input_choices: Vec<InputChoice>,
    #[serde(default)]
    pub additional_dictionaries: Option<AdditionalDictionariesOffer>,
    pub diagnostic_count: u64,
    pub options: ReadOptions,
    pub metadata: Option<DictionaryMetadata>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobEvent {
    pub snapshot: JobSnapshot,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportedSourceInfo {
    pub import_id: String,
    pub display_name: String,
    pub bytes: Option<u64>,
    pub is_directory: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceAttachment {
    pub source_id: String,
    pub display_name: String,
    pub bytes: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceDropEvent {
    Files { files: Vec<ImportedSourceInfo> },
    Error { error: AppError },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiagnosticPage {
    pub items: Vec<Diagnostic>,
    pub next_offset: Option<u64>,
    pub total: u64,
}

fn message_args_empty(args: &std::collections::BTreeMap<String, serde_json::Value>) -> bool {
    args.is_empty()
}
