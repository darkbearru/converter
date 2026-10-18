use crate::model::*;
use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub(super) type SharedReport = Arc<Mutex<ReportLog>>;
pub(super) struct ReportLog {
    path: PathBuf,
    writer: BufWriter<File>,
    pub warnings: u64,
    pub stages: Vec<StageProgress>,
    error: Option<ConverterError>,
}

pub(super) fn json_error(error: serde_json::Error) -> ConverterError {
    ConverterError::new("report_error", format!("Ошибка отчёта: {error}"))
}
fn poisoned() -> ConverterError {
    ConverterError::new(
        "report_error",
        "Не удалось получить доступ к журналу конвертации",
    )
}

impl ReportLog {
    pub fn create(path: &Path) -> Result<SharedReport> {
        Ok(Arc::new(Mutex::new(Self {
            path: path.into(),
            writer: BufWriter::new(File::create(path)?),
            warnings: 0,
            stages: Vec::new(),
            error: None,
        })))
    }
    pub fn check(shared: &SharedReport) -> Result<()> {
        match &shared.lock().map_err(|_| poisoned())?.error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
    pub fn context(shared: &SharedReport, ctx: &RunContext) -> RunContext {
        let mut context = ctx.clone();
        let log = shared.clone();
        let outer = ctx.progress.clone();
        context.progress = Arc::new(move |event| {
            if let Ok(mut log) = log.lock() {
                match &event {
                    CoreEvent::Diagnostic(diagnostic) => {
                        let result = serde_json::to_writer(&mut log.writer, diagnostic)
                            .map_err(json_error)
                            .and_then(|_| {
                                log.writer.write_all(b"\n").map_err(ConverterError::from)
                            });
                        match result {
                            Ok(()) => log.warnings += 1,
                            Err(error) => log.error = Some(error),
                        }
                    }
                    CoreEvent::StageCompleted(progress) => {
                        if let Some(previous) =
                            log.stages.iter_mut().find(|p| p.stage == progress.stage)
                        {
                            *previous = progress.clone();
                        } else {
                            log.stages.push(progress.clone());
                        }
                    }
                    _ => {}
                }
            }
            outer(event);
        });
        context
    }
    pub fn snapshot(shared: &SharedReport) -> Result<(u64, Vec<StageProgress>)> {
        Self::check(shared)?;
        let log = shared.lock().map_err(|_| poisoned())?;
        Ok((log.warnings, log.stages.clone()))
    }
    pub fn write(
        shared: &SharedReport,
        target: &Path,
        metadata: &DictionaryMetadata,
        summary: &ConversionSummary,
        ctx: &RunContext,
    ) -> Result<()> {
        let path = {
            let mut log = shared.lock().map_err(|_| poisoned())?;
            log.writer.flush()?;
            log.path.clone()
        };
        Self::check(shared)?;
        let mut out = BufWriter::new(File::create(target)?);
        out.write_all(b"{\"schema_version\":1,\"metadata\":")?;
        serde_json::to_writer(&mut out, metadata).map_err(json_error)?;
        out.write_all(b",\"summary\":")?;
        serde_json::to_writer(&mut out, summary).map_err(json_error)?;
        out.write_all(b",\"diagnostics\":[")?;
        let mut input = BufReader::new(File::open(path)?);
        let mut line = String::new();
        let mut first = true;
        loop {
            ctx.control.check()?;
            line.clear();
            if input.read_line(&mut line)? == 0 {
                break;
            }
            if !first {
                out.write_all(b",")?;
            }
            first = false;
            out.write_all(line.trim_end().as_bytes())?;
        }
        out.write_all(b"]}\n")?;
        out.flush()?;
        out.get_ref().sync_all()?;
        Ok(())
    }
}
