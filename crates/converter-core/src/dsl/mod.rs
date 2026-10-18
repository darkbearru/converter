pub mod decoder;
mod lexer;
mod metadata;
pub mod parser;

use crate::{formats::*, model::*};

pub struct DslReader;

impl DictionaryReader for DslReader {
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
        let trimmed = text.trim_start();
        let confidence = if trimmed.starts_with('<') || trimmed.contains('\0') {
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
            // Длинный начальный комментарий может занять весь ограниченный probe.
            20
        } else {
            0
        };
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
    fn read(
        &self,
        bundle: &InputBundle,
        options: &ReadOptions,
        sink: &mut dyn EntrySink,
        ctx: &RunContext,
    ) -> Result<ReadSummary> {
        let encoding = decoder::resolve_encoding(&bundle.main, options.encoding.clone(), ctx)?;
        parser::parse(bundle, &encoding, sink, ctx)
    }
}
