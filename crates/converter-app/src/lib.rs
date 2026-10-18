//! Очередь и состояния приложения без зависимости от WebView.
mod contracts;
pub mod jobs;
pub mod storage;
pub use contracts::*;
pub use jobs::manager::JobManager;
