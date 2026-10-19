use crate::{DownloadProgress, TransferState};
use std::{collections::VecDeque, sync::Arc, time::Duration};
use tokio::time::Instant;
pub fn download_percent(received: u64, total: Option<u64>) -> Option<f64> {
    total
        .filter(|n| *n > 0)
        .map(|n| (received as f64 / n as f64 * 100.0).min(100.0))
}
pub(crate) struct Progress {
    pub value: DownloadProgress,
    sink: Arc<dyn Fn(DownloadProgress) + Send + Sync>,
    samples: VecDeque<(Instant, u64)>,
}
impl Progress {
    pub fn new(id: String, sink: Arc<dyn Fn(DownloadProgress) + Send + Sync>) -> Self {
        Self {
            value: DownloadProgress {
                source_id: id,
                state: TransferState::Pending,
                received_bytes: 0,
                confirmed_bytes: 0,
                total_bytes: None,
                bytes_per_second: None,
                restart_reason: None,
                restart_message: None,
            },
            sink,
            samples: VecDeque::new(),
        }
    }
    pub fn reset(&mut self, bytes: u64) {
        self.samples.clear();
        self.value.received_bytes = bytes;
        self.value.confirmed_bytes = bytes;
        self.value.bytes_per_second = None;
        self.samples.push_back((Instant::now(), bytes));
    }
    pub fn emit(&mut self, state: TransferState) {
        self.value.state = state;
        if state == TransferState::Downloading {
            let now = Instant::now();
            self.samples.push_back((now, self.value.received_bytes));
            while self.samples.len() > 2
                && self
                    .samples
                    .get(1)
                    .is_some_and(|(at, _)| now.duration_since(*at) > Duration::from_secs(2))
            {
                self.samples.pop_front();
            }
            self.value.bytes_per_second = self.samples.front().and_then(|(at, bytes)| {
                let elapsed = now.duration_since(*at).as_secs_f64();
                (elapsed >= 0.01)
                    .then(|| self.value.received_bytes.saturating_sub(*bytes) as f64 / elapsed)
            });
        } else {
            self.samples.clear();
            self.value.bytes_per_second = None;
        }
        (self.sink)(self.value.clone());
    }
}
