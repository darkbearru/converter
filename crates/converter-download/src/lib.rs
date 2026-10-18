//! Устойчивое локальное хранилище и нативное скачивание источников.
mod cache;
mod checkpoint;
pub mod http;
mod model;
pub mod policy;
pub mod resume;
pub use cache::{DownloadCache, SourceGuard};
pub use checkpoint::{Checkpoint, ChunkHash, CommitPoint, ResumePoint};
pub use model::*;
