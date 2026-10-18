use crate::*;
use converter_core::model::{ConversionSummary, ConvertedArtifacts, PreparedFile, RunContext};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
pub mod desktop;
pub enum PlatformSelection {
    LocalPaths(Vec<PathBuf>),
    NativeHandles(Vec<String>),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublishedResult {
    pub id: String,
    pub directory: PathBuf,
    pub summary: ConversionSummary,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportChoice {
    Folder(PathBuf),
    DocumentHandle(String),
    ZipDocumentHandle(String),
}
#[async_trait::async_trait]
pub trait PlatformStorage: Send + Sync {
    async fn import(
        &self,
        selection: PlatformSelection,
        ctx: &RunContext,
    ) -> AppResult<Vec<PreparedFile>>;
    fn create_staging(&self, job_id: &str, output: &OutputChoice) -> AppResult<PathBuf>;
    async fn publish(
        &self,
        artifacts: ConvertedArtifacts,
        output: &OutputChoice,
        ctx: &RunContext,
    ) -> AppResult<PublishedResult>;
    async fn export(
        &self,
        result: &PublishedResult,
        choice: ExportChoice,
        ctx: &RunContext,
    ) -> AppResult<ExportSnapshot>;
}
