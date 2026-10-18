//! Устойчивое локальное хранилище и нативное скачивание источников.
mod cache;
mod checkpoint;
pub mod http;
mod model;
pub mod policy;
mod progress;
pub mod resume;
mod transfer;
pub use cache::{DownloadCache, SourceGuard};
pub use checkpoint::{Checkpoint, ChunkHash, CommitPoint, ResumePoint};
pub use model::*;
pub use progress::download_percent;
pub use transfer::{DownloadManager, TransferControl};
