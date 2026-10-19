use serde::{Deserialize, Serialize};
use std::{fmt, path::PathBuf, time::Duration};
use url::Url;

#[derive(Clone, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub source_id: String,
    pub url: Url,
    pub expected_sha256: Option<String>,
}
impl fmt::Debug for DownloadRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DownloadRequest")
            .field("source_id", &self.source_id)
            .finish_non_exhaustive()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CompletedDownload {
    pub source_id: String,
    pub path: PathBuf,
    pub display_name: String,
    pub bytes: u64,
    pub sha256: String,
    #[serde(default)]
    pub statistics: DownloadStatistics,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DownloadStatistics {
    pub resumes: u64,
    pub restarts: u64,
}
#[derive(Clone, Debug)]
pub struct DownloadPolicy {
    pub max_bytes: u64,
    pub checkpoint_bytes: u64,
    pub connect_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_redirects: u32,
    pub retry_delays: Vec<Duration>,
}
impl Default for DownloadPolicy {
    fn default() -> Self {
        Self {
            max_bytes: 32 * 1024 * 1024 * 1024,
            checkpoint_bytes: 1024 * 1024,
            connect_timeout: Duration::from_secs(15),
            idle_timeout: Duration::from_secs(30),
            max_redirects: 10,
            retry_delays: vec![
                Duration::from_secs(2),
                Duration::from_secs(5),
                Duration::from_secs(15),
            ],
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadError {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_key: Option<Box<str>>,
    #[serde(default, skip_serializing_if = "message_args_empty")]
    pub message_args: Box<std::collections::BTreeMap<String, serde_json::Value>>,
    pub code: String,
    pub message: String,
}
impl DownloadError {
    pub fn with_message(mut self, key: &str, args: serde_json::Value) -> Self {
        self.message_key = Some(key.into());
        self.message_args = serde_json::from_value(args).unwrap_or_default();
        self
    }

    pub fn new(code: &str, message: &str) -> Self {
        Self {
            message_key: None,
            message_args: Default::default(),
            code: code.into(),
            message: message.into(),
        }
    }
}
impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for DownloadError {}
impl From<std::io::Error> for DownloadError {
    fn from(value: std::io::Error) -> Self {
        Self::new(
            "download_io",
            &format!("Ошибка хранилища: {:?}", value.kind()),
        )
        .with_message(
            "messages.download_io",
            crate::message_args([("arg0", format!("{:?}", value.kind()))]),
        )
    }
}
pub type Result<T> = std::result::Result<T, DownloadError>;

#[derive(Clone, Serialize, Deserialize)]
pub struct DownloadManifest {
    pub version: u32,
    pub generation: u64,
    pub(crate) request: DownloadRequest,
    pub(crate) effective_url: Option<Url>,
    pub(crate) validator: Option<String>,
    pub(crate) total_bytes: Option<u64>,
    pub(crate) checkpoint: crate::Checkpoint,
    pub(crate) display_name: String,
    pub(crate) completed: Option<CompletedDownload>,
    #[serde(default)]
    pub(crate) transfer_finished: bool,
    #[serde(default)]
    pub(crate) statistics: DownloadStatistics,
}
impl fmt::Debug for DownloadManifest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DownloadManifest")
            .field("generation", &self.generation)
            .field("confirmed_bytes", &self.checkpoint.confirmed_bytes)
            .finish_non_exhaustive()
    }
}
impl DownloadManifest {
    /// Сохранённые счётчики; проверка байтов выполняется fetch перед использованием.
    pub fn confirmed_bytes(&self) -> u64 {
        self.checkpoint.confirmed_bytes
    }
    pub fn total_bytes(&self) -> Option<u64> {
        self.total_bytes
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransferState {
    Pending,
    Downloading,
    RetryWait,
    Paused,
    Verifying,
    Completed,
    Failed,
    Cancelled,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalizedMessage {
    pub message: String,
    pub message_key: Option<Box<str>>,
    #[serde(default)]
    pub message_args: Box<std::collections::BTreeMap<String, serde_json::Value>>,
}
impl LocalizedMessage {
    pub fn new(message: String, key: &str) -> Self {
        Self {
            message,
            message_key: Some(key.into()),
            message_args: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub source_id: String,
    pub state: TransferState,
    pub received_bytes: u64,
    pub confirmed_bytes: u64,
    pub total_bytes: Option<u64>,
    pub bytes_per_second: Option<f64>,
    pub restart_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart_message: Option<LocalizedMessage>,
}
#[derive(Clone, Debug)]
pub enum TransferOutcome {
    Completed(CompletedDownload),
    Paused,
    Cancelled,
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
