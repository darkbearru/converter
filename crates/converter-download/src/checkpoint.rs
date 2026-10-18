use serde::{Deserialize, Serialize};
use url::Url;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Checkpoint {
    pub confirmed_bytes: u64,
    pub chunk_hashes: Vec<ChunkHash>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChunkHash {
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Clone)]
pub struct ResumePoint {
    pub confirmed_bytes: u64,
    pub validator: Option<String>,
    pub effective_url: Option<Url>,
    pub total_bytes: Option<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitPoint {
    BeforeSync,
    AfterSync,
    BeforeManifest,
    AfterManifest,
}
