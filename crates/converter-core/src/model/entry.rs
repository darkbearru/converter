use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreparedFile {
    pub id: String,
    pub display_name: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputBundle {
    pub main: PreparedFile,
    pub abbreviations: Option<PreparedFile>,
    pub resources: Vec<ResourceRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceRecord {
    pub key: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DictionaryMetadata {
    pub title: String,
    pub description: String,
    pub index_language: String,
    pub contents_language: String,
    pub source_encoding: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DictionaryEntry {
    pub id: u64,
    pub source_line: u64,
    pub parent_id: Option<u64>,
    pub headwords: Vec<String>,
    pub display_headword: String,
    pub html: String,
    pub resource_keys: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MdxRecord {
    pub key: String,
    pub html: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WriteSummary {
    pub entries: u64,
    pub key_blocks: u64,
    pub record_blocks: u64,
    pub total_bytes: u64,
}
