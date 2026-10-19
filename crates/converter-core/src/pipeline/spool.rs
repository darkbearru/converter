use super::{
    link_index::LinkIndex,
    report::{ReportLog, SharedReport},
    sort::{ExternalSort, SortedRecords},
};
use crate::{formats::EntrySink, model::*};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

pub(super) struct EntrySpool {
    bodies: BufWriter<File>,
    offset: u64,
    sorter: ExternalSort,
    links: LinkIndex,
    report: SharedReport,
    ctx: RunContext,
    pub articles: u64,
    pub keys: u64,
}

impl EntrySpool {
    pub fn new(directory: &Path, report: SharedReport, ctx: &RunContext) -> Result<Self> {
        let bodies = directory.join("articles.bin");
        Ok(Self {
            bodies: BufWriter::new(File::create(&bodies)?),
            offset: 0,
            sorter: ExternalSort::new(&directory.join("sort"), &bodies, ctx)?,
            links: LinkIndex::new(&directory.join("links.sqlite"))?,
            report,
            ctx: ctx.clone(),
            articles: 0,
            keys: 0,
        })
    }
    pub fn finish(&mut self, source: &str) -> Result<(SortedRecords, SortMetrics)> {
        self.bodies.flush()?;
        self.links.missing(&self.ctx, source, |diagnostic| {
            self.ctx.emit(CoreEvent::Diagnostic(diagnostic));
            ReportLog::check(&self.report)
        })?;
        let records = self.sorter.finish()?;
        Ok((records, self.sorter.metrics.clone()))
    }
}

impl EntrySink for EntrySpool {
    fn push(&mut self, entry: DictionaryEntry) -> Result<()> {
        self.ctx.control.check()?;
        ReportLog::check(&self.report)?;
        if entry.headwords.is_empty() {
            return Err(ConverterError::new(
                "invalid_entry",
                "Читатель формата вернул статью без ключей",
            )
            .with_message("messages.invalid_entry", crate::model::message_args([])));
        }
        if entry.html.len() > self.ctx.limits.max_html_bytes {
            return Err(ConverterError::new(
                "article_too_large",
                "HTML статьи превышает допустимый размер",
            )
            .with_message("messages.article_too_large", crate::model::message_args([])));
        }
        for key in entry.headwords {
            self.links.key(&key)?;
            self.sorter
                .push(key, self.offset, entry.html.len() as u64)?;
            self.keys = self.keys.checked_add(1).ok_or_else(|| {
                ConverterError::new("size_overflow", "Переполнение числа ключей")
                    .with_message("messages.size_overflow", crate::model::message_args([]))
            })?;
        }
        self.links.references(&entry.html, entry.source_line)?;
        self.bodies.write_all(entry.html.as_bytes())?;
        self.offset = self
            .offset
            .checked_add(entry.html.len() as u64)
            .ok_or_else(|| {
                ConverterError::new("size_overflow", "Переполнение промежуточного файла")
                    .with_message("messages.size_overflow_2", crate::model::message_args([]))
            })?;
        self.articles += 1;
        Ok(())
    }
    fn diagnostic(&mut self, diagnostic: Diagnostic) -> Result<()> {
        self.ctx.emit(CoreEvent::Diagnostic(diagnostic));
        ReportLog::check(&self.report)
    }
}
