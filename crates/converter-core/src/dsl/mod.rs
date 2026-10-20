mod abbreviations;
mod annotations;
mod code_pages;
pub mod decoder;
pub mod headwords;
pub(crate) mod includes;
mod lexer;
mod markup;
mod metadata;
pub mod parser;
mod references;
mod relationships;
pub mod renderer;

use crate::{formats::*, model::*};

pub struct DslReader;

fn dsl_stem(name: &str) -> &str {
    name.strip_suffix(".dz")
        .unwrap_or(name)
        .strip_suffix(".dsl")
        .unwrap_or(name)
}

fn probe_text(text: &str, name: &str) -> u8 {
    let trimmed = text.trim_start();
    if trimmed.starts_with('<') || trimmed.contains('\0') {
        0
    } else if trimmed.starts_with("#NAME ") || trimmed.starts_with("#INDEX_LANGUAGE ") {
        100
    } else if trimmed
        .lines()
        .any(|line| line.starts_with([' ', '\t']) && !line.trim().is_empty())
    {
        40
    } else if name.to_ascii_lowercase().ends_with(".dsl")
        || name.to_ascii_lowercase().ends_with(".dsl.dz")
    {
        20
    } else {
        0
    }
}

impl DictionaryReader for DslReader {
    fn is_auxiliary(&self, name: &str) -> bool {
        dsl_stem(&name.to_lowercase()).ends_with("_abrv")
    }
    fn is_companion(&self, main: &str, candidate: &str) -> bool {
        let candidate = candidate.to_lowercase();
        (candidate.ends_with(".dsl") || candidate.ends_with(".dsl.dz"))
            && dsl_stem(&candidate) == format!("{}_abrv", dsl_stem(&main.to_lowercase()))
    }
    fn descriptor(&self) -> FormatDescriptor {
        FormatDescriptor {
            id: "dsl".into(),
            name: "Lingvo DSL".into(),
            extensions: vec!["dsl".into(), "dsl.dz".into()],
            supports_resources: true,
        }
    }
    fn probe(&self, prefix: &[u8], name: &str) -> ProbeResult {
        if prefix.starts_with(&[0x1f, 0x8b]) {
            return ProbeResult { confidence: 80 };
        }
        let (encoding, skip) = if prefix.starts_with(&[0xff, 0xfe]) {
            (encoding_rs::UTF_16LE, 2)
        } else if prefix.starts_with(&[0xfe, 0xff]) {
            (encoding_rs::UTF_16BE, 2)
        } else {
            (
                encoding_rs::UTF_8,
                usize::from(prefix.starts_with(&[0xef, 0xbb, 0xbf])) * 3,
            )
        };
        let text = encoding.decode_without_bom_handling(&prefix[skip..]).0;
        let mut confidence = probe_text(&text, name);
        if confidence == 0 && skip == 0 && prefix.contains(&0) {
            // Это лишь допуск к выбору кодировки: Auto всё ещё требует её подтверждения.
            for candidate in [encoding_rs::UTF_16LE, encoding_rs::UTF_16BE] {
                let (decoded, malformed) = candidate.decode_without_bom_handling(prefix);
                let score = probe_text(&decoded, name);
                if !malformed && score >= 40 {
                    confidence = confidence.max(score);
                }
            }
        }
        ProbeResult { confidence }
    }
    fn inspect(
        &self,
        bundle: &InputBundle,
        options: &ReadOptions,
        ctx: &RunContext,
    ) -> Result<DictionaryMetadata> {
        let encoding = decoder::resolve_encoding(&bundle.main, options.encoding.clone(), ctx)?;
        metadata::inspect_metadata(bundle, &encoding, ctx)
    }
    fn relationships(
        &self,
        bundle: &InputBundle,
        options: &ReadOptions,
        ctx: &RunContext,
    ) -> Result<Option<SourceRelationships>> {
        relationships::inspect(bundle, options, ctx).map(Some)
    }
    fn read(
        &self,
        bundle: &InputBundle,
        options: &ReadOptions,
        sink: &mut dyn EntrySink,
        ctx: &RunContext,
    ) -> Result<ReadSummary> {
        let encoding = decoder::resolve_encoding(&bundle.main, options.encoding.clone(), ctx)?;
        let mut rendered = RenderSink {
            bundle,
            options,
            ctx,
            sink,
            articles: 0,
            warnings: 0,
            cache: renderer::RenderCache::with_dictionaries(metadata::dictionary_names(
                bundle, options, ctx,
            )?),
            missing_abbreviations_reported: false,
        };
        let mut summary = parser::parse_raw(bundle, &encoding, &mut rendered, ctx)?;
        summary.warnings += rendered.warnings;
        ctx.emit(CoreEvent::StageCompleted(StageProgress {
            stage: ConversionStage::Read,
            completed: summary.source_articles,
            total: Some(summary.source_articles),
            unit: ProgressUnit::Entries,
            articles: summary.source_articles,
            resources: 0,
        }));
        Ok(summary)
    }
}

struct RenderSink<'a> {
    bundle: &'a InputBundle,
    options: &'a ReadOptions,
    ctx: &'a RunContext,
    sink: &'a mut dyn EntrySink,
    articles: u64,
    warnings: u64,
    cache: renderer::RenderCache,
    missing_abbreviations_reported: bool,
}

impl parser::AstSink for RenderSink<'_> {
    fn push(&mut self, ast: parser::ArticleAst) -> Result<()> {
        let mut headwords = Vec::new();
        let first = ast.headwords.first().map_or("", String::as_str);
        for (index, raw) in ast.headwords.iter().enumerate() {
            let raw = if index == 0 {
                raw.clone()
            } else {
                references::substitute_tilde(raw, first)
            };
            for key in headwords::expand_headwords(&raw, self.ctx.limits.max_headword_variants)? {
                if !headwords.contains(&key) {
                    headwords.push(key);
                }
                if headwords.len() > self.ctx.limits.max_headword_variants {
                    return Err(ConverterError::new(
                        "headword_variants_limit",
                        "Превышен лимит вариантов всех заголовков статьи",
                    )
                    .with_message(
                        "messages.headword_variants_limit",
                        crate::model::message_args([]),
                    ));
                }
            }
        }
        let rendered = renderer::render_with_context(
            &ast,
            self.bundle,
            self.options,
            self.ctx,
            &mut self.cache,
        )?;
        if rendered.uses_abbreviations
            && self.bundle.abbreviations.is_none()
            && !self.missing_abbreviations_reported
        {
            self.sink.diagnostic(Diagnostic {
                message_key: Some("messages.missing_abbreviations_file".into()),
                message_args: serde_json::from_value(crate::model::message_args([]))
                    .unwrap_or_default(),
                code: "missing_abbreviations_file".into(),
                message: "Файл сокращений не предоставлен: пометы сохранены без расшифровок".into(),
                source_location: Some(SourceLocation {
                    source: ast.source_name.clone(),
                    line: ast.source_line,
                }),
                severity: DiagnosticSeverity::Info,
                occurrences: 1,
            })?;
            self.missing_abbreviations_reported = true;
        }
        self.warnings += rendered
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
            .map(|diagnostic| diagnostic.occurrences)
            .sum::<u64>();
        for diagnostic in rendered.diagnostics {
            self.sink.diagnostic(diagnostic)?;
        }
        self.sink.push(DictionaryEntry {
            source_name: ast.source_name.clone(),
            id: ast.id,
            source_line: ast.source_line,
            parent_id: ast.parent_id,
            headwords,
            display_headword: rendered.display_headword,
            html: rendered.html,
            resource_keys: rendered.resource_keys,
        })?;
        self.articles += 1;
        self.ctx.emit(CoreEvent::Progress(StageProgress {
            stage: ConversionStage::Read,
            completed: self.articles,
            total: None,
            unit: ProgressUnit::Entries,
            articles: self.articles,
            resources: 0,
        }));
        Ok(())
    }
    fn diagnostic(&mut self, diagnostic: Diagnostic) -> Result<()> {
        self.sink.diagnostic(diagnostic)
    }
}
