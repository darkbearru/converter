use crate::model::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FormatDescriptor {
    pub id: String,
    pub name: String,
    pub extensions: Vec<String>,
    pub supports_resources: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ProbeResult {
    pub confidence: u8,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReadSummary {
    pub source_articles: u64,
    pub warnings: u64,
}

#[derive(Clone, Debug)]
pub struct SourceRelationships {
    pub title: String,
    pub external_dictionaries: Vec<String>,
    pub source_articles: u64,
}

pub trait EntrySink {
    fn push(&mut self, entry: DictionaryEntry) -> Result<()>;
    fn diagnostic(&mut self, diagnostic: Diagnostic) -> Result<()>;
}

pub trait DictionaryReader: Send + Sync {
    fn descriptor(&self) -> FormatDescriptor;
    fn is_auxiliary(&self, _name: &str) -> bool {
        false
    }
    fn is_companion(&self, _main: &str, _candidate: &str) -> bool {
        false
    }
    fn probe(&self, prefix: &[u8], name: &str) -> ProbeResult;
    fn inspect(
        &self,
        bundle: &InputBundle,
        options: &ReadOptions,
        ctx: &RunContext,
    ) -> Result<DictionaryMetadata>;
    fn relationships(
        &self,
        _bundle: &InputBundle,
        _options: &ReadOptions,
        _ctx: &RunContext,
    ) -> Result<Option<SourceRelationships>> {
        Ok(None)
    }
    fn read(
        &self,
        bundle: &InputBundle,
        options: &ReadOptions,
        sink: &mut dyn EntrySink,
        ctx: &RunContext,
    ) -> Result<ReadSummary>;
}
