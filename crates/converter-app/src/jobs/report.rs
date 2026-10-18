use crate::AppResult;
use converter_core::model::{ConvertedArtifacts, PreparedFile, ReadOptions, RunContext};
use converter_download::CompletedDownload;
use std::{
    fs::File,
    io::{self, Read, Write},
};
pub enum SourceAudit {
    Local {
        kind: &'static str,
        display_name: String,
        bytes: Option<u64>,
    },
    Remote {
        kind: &'static str,
        source: CompletedDownload,
    },
}
impl SourceAudit {
    pub fn local(file: &PreparedFile) -> AppResult<Self> {
        let meta = std::fs::metadata(&file.path)?;
        Ok(Self::Local {
            kind: "local",
            display_name: file.display_name.clone(),
            bytes: meta.is_file().then_some(meta.len()),
        })
    }
    pub fn remote(source: CompletedDownload) -> Self {
        Self::Remote {
            kind: "remote",
            source,
        }
    }
}
pub fn augment(
    artifacts: &ConvertedArtifacts,
    sources: &[SourceAudit],
    options: &ReadOptions,
    ctx: &RunContext,
) -> AppResult<()> {
    // Переписывается потоково; массив diagnostics никогда не загружается целиком.
    atomicwrites::AtomicFile::new(&artifacts.report, atomicwrites::OverwriteBehavior::AllowOverwrite).write(|out| -> io::Result<()> {
        let mut input = File::open(&artifacts.report)?; let mut first = [0u8; 1]; input.read_exact(&mut first)?;
        if first != *b"{" { return Err(io::Error::new(io::ErrorKind::InvalidData, "Некорректный отчёт")); }
        out.write_all(b"{")?;
        // Публичные поля перечислены явно, чтобы локальный путь кэша не попал в отчёт.
        let source_values: Vec<_> = sources.iter().map(|source| match source {
            SourceAudit::Local { kind, display_name, bytes } => serde_json::json!({"kind": kind, "display_name": display_name, "bytes": bytes}),
            SourceAudit::Remote { source, kind } => serde_json::json!({"kind": kind, "source_id": source.source_id, "display_name": source.display_name, "bytes": source.bytes, "sha256": source.sha256, "statistics": source.statistics}),
        }).collect();
        let header = serde_json::json!({"application": {"name": "Converter", "version": env!("CARGO_PKG_VERSION"), "mdict_version": "2.0"}, "sources": source_values, "options": options});
        let serialized = serde_json::to_vec(&header).map_err(io::Error::other)?;
        out.write_all(&serialized[1..serialized.len()-1])?; out.write_all(b",")?;
        let mut buffer = [0u8; 65536]; loop { ctx.control.check().map_err(io::Error::other)?; let n = input.read(&mut buffer)?; if n == 0 { break; } out.write_all(&buffer[..n])?; }
        Ok(())
    }).map_err(|_| crate::AppError::new("report_write_failed", "Не удалось сохранить происхождение источников в отчёте"))
}
