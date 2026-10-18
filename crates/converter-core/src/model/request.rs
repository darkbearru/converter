use super::{DictionaryMetadata, InputBundle, StageProgress};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncodingChoice {
    #[default]
    Auto,
    Manual(String),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReadOptions {
    pub encoding: EncodingChoice,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SortMetrics {
    pub merge_passes: u32,
    pub peak_sort_bytes: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConversionSummary {
    pub source_articles: u64,
    pub output_keys: u64,
    pub resources: u64,
    pub warnings: u64,
    pub stages: Vec<StageProgress>,
    pub sort_metrics: SortMetrics,
}

#[derive(Clone, Debug)]
pub struct ConversionRequest {
    pub bundle: InputBundle,
    pub options: ReadOptions,
    pub metadata_override: Option<DictionaryMetadata>,
    pub staging_dir: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConvertedArtifacts {
    pub directory: PathBuf,
    pub mdx: PathBuf,
    pub mdd: PathBuf,
    pub report: PathBuf,
    pub summary: ConversionSummary,
}
