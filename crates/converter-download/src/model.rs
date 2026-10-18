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
    pub code: String,
    pub message: String,
}
impl DownloadError {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
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
}
impl fmt::Debug for DownloadManifest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DownloadManifest")
            .field("generation", &self.generation)
            .field("confirmed_bytes", &self.checkpoint.confirmed_bytes)
            .finish_non_exhaustive()
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
pub struct DownloadProgress {
    pub source_id: String,
    pub state: TransferState,
    pub received_bytes: u64,
    pub confirmed_bytes: u64,
    pub total_bytes: Option<u64>,
    pub bytes_per_second: Option<f64>,
    pub restart_reason: Option<String>,
}
#[derive(Clone, Debug)]
pub enum TransferOutcome {
    Completed(CompletedDownload),
    Paused,
    Cancelled,
}
