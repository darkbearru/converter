use crate::model::*;
use rusqlite::{Connection, params};
use serde::Serialize;
use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use unicode_normalization::UnicodeNormalization;

pub(super) type SharedReport = Arc<Mutex<ReportLog>>;
pub(super) struct ReportLog {
    path: PathBuf,
    writer: BufWriter<File>,
    groups: Connection,
    records: i64,
    last_emitted: i64,
    counts: DiagnosticCounts,
    pub stages: Vec<StageProgress>,
    error: Option<ConverterError>,
}

#[derive(Default, Serialize)]
struct DiagnosticCounts {
    warning_groups: u64,
    info_groups: u64,
    warning_occurrences: u64,
    info_occurrences: u64,
}

// При ошибке или отмене уже обнаруженные предупреждения тоже доходят до журнала задачи.
pub(super) struct ReportNotifications {
    report: SharedReport,
    context: RunContext,
}

impl ReportNotifications {
    pub fn finish(&self) -> Result<()> {
        ReportLog::emit_warnings(&self.report, &self.context)
    }
}

impl Drop for ReportNotifications {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

pub(super) fn json_error(error: serde_json::Error) -> ConverterError {
    ConverterError::new("report_error", format!("Ошибка отчёта: {error}"))
}
fn sql_error(error: rusqlite::Error) -> ConverterError {
    ConverterError::new(
        "report_error",
        format!("Ошибка индекса диагностики: {error}"),
    )
}

fn group_key(diagnostic: &Diagnostic, ordinal: i64) -> Result<String> {
    let subject = match diagnostic.code.as_str() {
        "missing_resource" => Some(
            diagnostic
                .message
                .strip_prefix("Отсутствует ресурс: ")
                .unwrap_or(&diagnostic.message)
                .replace('\\', "/")
                .trim_start_matches('/')
                .to_lowercase()
                .nfc()
                .collect::<String>(),
        ),
        "missing_abbreviation" => Some(diagnostic.message.clone()),
        _ if diagnostic.severity == DiagnosticSeverity::Info => Some(diagnostic.message.clone()),
        _ => None,
    };
    serde_json::to_string(&(
        diagnostic.severity,
        &diagnostic.code,
        diagnostic
            .source_location
            .as_ref()
            .map(|location| &location.source),
        subject,
        // Остальные предупреждения сохраняют прежнюю детализацию, без общего подавления.
        if matches!(
            diagnostic.code.as_str(),
            "missing_resource" | "missing_abbreviation"
        ) || diagnostic.severity == DiagnosticSeverity::Info
        {
            None
        } else {
            Some(ordinal)
        },
    ))
    .map_err(json_error)
}
fn poisoned() -> ConverterError {
    ConverterError::new(
        "report_error",
        "Не удалось получить доступ к журналу конвертации",
    )
}

impl ReportLog {
    pub fn create(path: &Path) -> Result<SharedReport> {
        let groups = Connection::open(path.with_extension("sqlite")).map_err(sql_error)?;
        // Индекс и сортировка остаются на диске даже при миллионах сообщений.
        groups.execute_batch("PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF; PRAGMA temp_store=FILE; PRAGMA cache_size=-1024;
            CREATE TABLE diagnostics (group_key TEXT PRIMARY KEY, ordinal INTEGER NOT NULL UNIQUE, diagnostic TEXT NOT NULL, severity TEXT NOT NULL, occurrences INTEGER NOT NULL);
            CREATE INDEX warning_order ON diagnostics(severity, ordinal); BEGIN;").map_err(sql_error)?;
        Ok(Arc::new(Mutex::new(Self {
            path: path.into(),
            writer: BufWriter::new(File::create(path)?),
            groups,
            records: 0,
            last_emitted: 0,
            counts: DiagnosticCounts::default(),
            stages: Vec::new(),
            error: None,
        })))
    }

    fn record(&mut self, diagnostic: &Diagnostic) -> Result<()> {
        let ordinal = self.records.checked_add(1).ok_or_else(|| {
            ConverterError::new("report_error", "Переполнение числа сообщений диагностики")
        })?;
        let occurrences = i64::try_from(diagnostic.occurrences)
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| {
                ConverterError::new("report_error", "Переполнение числа вхождений диагностики")
            })?;
        let raw = serde_json::to_string(diagnostic).map_err(json_error)?;
        self.writer.write_all(raw.as_bytes())?;
        self.writer.write_all(b"\n")?;
        let severity = if diagnostic.severity == DiagnosticSeverity::Warning {
            "warning"
        } else {
            "info"
        };
        let combined: i64 = self.groups.prepare_cached("INSERT INTO diagnostics VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(group_key) DO UPDATE SET occurrences=diagnostics.occurrences+excluded.occurrences RETURNING occurrences")
            .map_err(sql_error)?
            .query_row(params![group_key(diagnostic, ordinal)?, ordinal, raw, severity, occurrences], |row| row.get(0))
            .map_err(sql_error)?;
        let inserted = u64::from(combined == occurrences);
        match diagnostic.severity {
            DiagnosticSeverity::Info => {
                self.counts.info_groups += inserted;
                self.counts.info_occurrences += occurrences as u64;
            }
            DiagnosticSeverity::Warning => {
                self.counts.warning_groups += inserted;
                self.counts.warning_occurrences += occurrences as u64;
            }
        }
        self.records = ordinal;
        Ok(())
    }

    pub fn notifications(shared: &SharedReport, ctx: &RunContext) -> ReportNotifications {
        ReportNotifications {
            report: shared.clone(),
            context: ctx.clone(),
        }
    }

    fn emit_warnings(shared: &SharedReport, ctx: &RunContext) -> Result<()> {
        let mut log = shared.lock().map_err(|_| poisoned())?;
        let Self {
            groups,
            last_emitted,
            ..
        } = &mut *log;
        let mut query = groups.prepare("SELECT ordinal, diagnostic, occurrences FROM diagnostics WHERE severity='warning' AND ordinal>?1 ORDER BY ordinal").map_err(sql_error)?;
        let mut rows = query.query([*last_emitted]).map_err(sql_error)?;
        while let Some(row) = rows.next().map_err(sql_error)? {
            let ordinal = row.get(0).map_err(sql_error)?;
            let mut diagnostic: Diagnostic =
                serde_json::from_str(&row.get::<_, String>(1).map_err(sql_error)?)
                    .map_err(json_error)?;
            diagnostic.occurrences = row.get::<_, i64>(2).map_err(sql_error)? as u64;
            ctx.emit(CoreEvent::Diagnostic(diagnostic));
            *last_emitted = ordinal;
        }
        Ok(())
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
                        if log.error.is_none()
                            && let Err(error) = log.record(diagnostic)
                        {
                            log.error = Some(error);
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
            if !matches!(event, CoreEvent::Diagnostic(_)) {
                outer(event);
            }
        });
        context
    }
    pub fn snapshot(shared: &SharedReport) -> Result<(u64, Vec<StageProgress>)> {
        Self::check(shared)?;
        let log = shared.lock().map_err(|_| poisoned())?;
        Ok((log.counts.warning_groups, log.stages.clone()))
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
        out.write_all(b"{\"schema_version\":2,\"metadata\":")?;
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
        out.write_all(b"],\"grouped_diagnostics\":[")?;
        let log = shared.lock().map_err(|_| poisoned())?;
        let mut query = log
            .groups
            .prepare("SELECT diagnostic, occurrences FROM diagnostics ORDER BY ordinal")
            .map_err(sql_error)?;
        let mut rows = query.query([]).map_err(sql_error)?;
        let mut first = true;
        while let Some(row) = rows.next().map_err(sql_error)? {
            ctx.control.check()?;
            let mut diagnostic: Diagnostic =
                serde_json::from_str(&row.get::<_, String>(0).map_err(sql_error)?)
                    .map_err(json_error)?;
            diagnostic.occurrences = row.get::<_, i64>(1).map_err(sql_error)? as u64;
            if !first {
                out.write_all(b",")?;
            }
            first = false;
            serde_json::to_writer(&mut out, &diagnostic).map_err(json_error)?;
        }
        out.write_all(b"],\"diagnostic_counts\":")?;
        serde_json::to_writer(&mut out, &log.counts).map_err(json_error)?;
        out.write_all(b"}\n")?;
        out.flush()?;
        out.get_ref().sync_all()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn группировка_сохраняет_границы_источников_и_отдельные_неизвестные_теги()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let report = ReportLog::create(&temp.path().join("diagnostics.jsonl"))?;
        let mut outer = RunContext::new(temp.path().into());
        let events = Arc::new(Mutex::new(Vec::new()));
        let received = events.clone();
        outer.progress = Arc::new(move |event| {
            if let CoreEvent::Diagnostic(diagnostic) = event {
                received.lock().expect("Сообщения").push(diagnostic);
            }
        });
        let inner = ReportLog::context(&report, &outer);
        for (source, line, name) in [
            ("a.dsl", 1, "café.png"),
            ("a.dsl", 3, "CAFE\u{301}.PNG"),
            ("b.dsl", 1, "café.png"),
        ] {
            inner.emit(CoreEvent::Diagnostic(Diagnostic {
                code: "missing_resource".into(),
                message: format!("Отсутствует ресурс: {name}"),
                source_location: Some(SourceLocation {
                    source: source.into(),
                    line,
                }),
                severity: DiagnosticSeverity::Warning,
                occurrences: 1,
            }));
        }
        for line in [5, 7] {
            inner.emit(CoreEvent::Diagnostic(Diagnostic {
                code: "unknown_tag".into(),
                message: "Неизвестный тег: [other]".into(),
                source_location: Some(SourceLocation {
                    source: "a.dsl".into(),
                    line,
                }),
                severity: DiagnosticSeverity::Warning,
                occurrences: 1,
            }));
        }
        assert!(events.lock().expect("Сообщения").is_empty());
        let notifications = ReportLog::notifications(&report, &outer);
        notifications.finish()?;
        drop(notifications);
        let events = events.lock().expect("Сообщения");
        assert_eq!(events.len(), 4);
        assert_eq!(
            events.iter().map(|d| d.occurrences).collect::<Vec<_>>(),
            vec![2, 1, 1, 1]
        );
        assert_eq!(
            events[0]
                .source_location
                .as_ref()
                .expect("Первый источник")
                .source,
            "a.dsl"
        );
        assert_eq!(
            events[1]
                .source_location
                .as_ref()
                .expect("Второй источник")
                .source,
            "b.dsl"
        );
        assert_eq!(ReportLog::snapshot(&report)?.0, 4);
        Ok(())
    }
}
