use super::{ConverterError, Diagnostic, Limits, Result};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Debug, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversionStage {
    Inspect,
    Read,
    Sort,
    WriteMdx,
    WriteMdd,
    Validate,
    Publish,
}

#[derive(Clone, Debug, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressUnit {
    Bytes,
    Entries,
    Resources,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StageProgress {
    pub stage: ConversionStage,
    pub completed: u64,
    pub total: Option<u64>,
    pub unit: ProgressUnit,
    pub articles: u64,
    pub resources: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum CoreEvent {
    Progress(StageProgress),
    Diagnostic(Diagnostic),
    StageCompleted(StageProgress),
}

#[derive(Clone, Default)]
pub struct RunControl(Arc<AtomicBool>);

impl RunControl {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn check(&self) -> Result<()> {
        if self.0.load(Ordering::Acquire) {
            Err(ConverterError::new("cancelled", "Операция отменена"))
        } else {
            Ok(())
        }
    }
}

#[derive(Clone)]
pub struct RunContext {
    pub scratch_dir: PathBuf,
    pub limits: Limits,
    pub control: RunControl,
    pub progress: Arc<dyn Fn(CoreEvent) + Send + Sync>,
}

impl RunContext {
    pub fn new(scratch_dir: PathBuf) -> Self {
        Self {
            scratch_dir,
            limits: Limits::default(),
            control: RunControl::default(),
            progress: Arc::new(|_| {}),
        }
    }
    pub fn emit(&self, event: CoreEvent) {
        (self.progress)(event);
    }
}
