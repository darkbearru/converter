//! Устойчивое локальное хранилище и нативное скачивание источников.
mod cache;
mod checkpoint;
mod model;
pub use cache::{DownloadCache, SourceGuard};
pub use checkpoint::{Checkpoint, ChunkHash, CommitPoint, ResumePoint};
pub use model::*;
