use crate::{DownloadJobProgress, DownloadKind};
use converter_download::{DownloadProgress, TransferState, download_percent};
use std::time::Duration;
pub fn aggregate_downloads(files: Vec<DownloadProgress>) -> DownloadJobProgress {
    let total_bytes =
        files.iter().try_fold(0u64, |total, file| {
            total.checked_add(file.total_bytes.or_else(|| {
                (file.state == TransferState::Completed).then_some(file.received_bytes)
            })?)
        });
    let received_bytes = files.iter().map(|f| f.received_bytes).sum();
    let confirmed_bytes = files.iter().map(|f| f.confirmed_bytes).sum();
    DownloadJobProgress {
        kind: if files.is_empty() {
            DownloadKind::Local
        } else {
            DownloadKind::Remote
        },
        files,
        received_bytes,
        confirmed_bytes,
        total_bytes: if total_bytes == Some(0) {
            None
        } else {
            total_bytes
        },
        percent: download_percent(received_bytes, total_bytes),
    }
}
#[derive(Default)]
pub struct ProgressGate {
    last: Option<Duration>,
    key: String,
}
impl ProgressGate {
    pub fn should_emit(&mut self, now: Duration, key: &str, terminal: bool) -> bool {
        if terminal
            || self.key != key
            || self
                .last
                .is_none_or(|last| now.saturating_sub(last) >= Duration::from_millis(200))
        {
            self.last = Some(now);
            self.key = key.into();
            true
        } else {
            false
        }
    }
}
