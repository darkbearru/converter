use super::validate::validate_artifacts;
use crate::model::*;
use serde::Deserialize;
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};

fn basename(value: &str) -> String {
    let value: String = value
        .chars()
        .take(60)
        .map(|ch| {
            if ch.is_control() || "/\\:*?\"<>|".contains(ch) {
                '-'
            } else {
                ch
            }
        })
        .collect();
    let value = value.trim_matches([' ', '.']);
    if value.is_empty() {
        "dictionary".into()
    } else {
        value.into()
    }
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android"
))]
fn rename_exclusive(from: &Path, to: &Path) -> io::Result<()> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        from,
        rustix::fs::CWD,
        to,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(io::Error::from)
}
#[cfg(windows)]
fn rename_exclusive(from: &Path, to: &Path) -> io::Result<()> {
    atomicwrites::move_atomic(from, to)
}
#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android",
    windows
)))]
fn rename_exclusive(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Атомарная публикация не поддерживается на этой платформе",
    ))
}

pub fn publish_local(
    artifacts: ConvertedArtifacts,
    destination: &Path,
    name: &str,
    ctx: &RunContext,
) -> Result<PathBuf> {
    ctx.control.check()?;
    if std::fs::symlink_metadata(&artifacts.directory)?
        .file_type()
        .is_symlink()
    {
        return Err(ConverterError::new(
            "invalid_staging",
            "Staging не может быть символической ссылкой",
        ));
    }
    let source = std::fs::canonicalize(&artifacts.directory)?;
    let destination = std::fs::canonicalize(destination)?;
    if !destination.is_dir() {
        return Err(ConverterError::new(
            "invalid_output",
            "Место сохранения должно быть папкой",
        ));
    }
    let expected = [&artifacts.mdx, &artifacts.mdd, &artifacts.report];
    for path in expected {
        let metadata = std::fs::symlink_metadata(path)?;
        if !metadata.is_file() || std::fs::canonicalize(path)?.parent() != Some(source.as_path()) {
            return Err(ConverterError::new(
                "invalid_staging",
                "Результат должен содержать обычные файлы в одном staging-каталоге",
            ));
        }
    }
    let mut count = 0;
    for entry in std::fs::read_dir(&source)? {
        entry?;
        count += 1;
    }
    if count != expected.len() {
        return Err(ConverterError::new(
            "invalid_staging",
            "Staging содержит посторонние файлы",
        ));
    }
    validate_artifacts(&artifacts, ctx)?;
    let mut report = serde_json::Deserializer::from_reader(File::open(&artifacts.report)?);
    serde::de::IgnoredAny::deserialize(&mut report)
        .map_err(|e| ConverterError::new("invalid_report", format!("Повреждён отчёт: {e}")))?;
    report.end().map_err(|e| {
        ConverterError::new("invalid_report", format!("Лишние данные в отчёте: {e}"))
    })?;
    drop(report);
    let base = basename(name);
    for suffix in 0..10_000u32 {
        ctx.control.check()?;
        let name = if suffix == 0 {
            format!("{base}-converted")
        } else {
            format!("{base}-converted-{suffix}")
        };
        let target = destination.join(name);
        ctx.emit(CoreEvent::Progress(StageProgress {
            stage: ConversionStage::Publish,
            completed: 0,
            total: Some(1),
            unit: ProgressUnit::Entries,
            articles: artifacts.summary.source_articles,
            resources: artifacts.summary.resources,
        }));
        ctx.control.check()?;
        match rename_exclusive(&source, &target) {
            Ok(()) => {
                ctx.emit(CoreEvent::StageCompleted(StageProgress {
                    stage: ConversionStage::Publish,
                    completed: 1,
                    total: Some(1),
                    unit: ProgressUnit::Entries,
                    articles: artifacts.summary.source_articles,
                    resources: artifacts.summary.resources,
                }));
                return Ok(target);
            }
            Err(error)
                if error.kind() == io::ErrorKind::AlreadyExists
                    || std::fs::symlink_metadata(&target).is_ok() =>
            {
                continue;
            }
            Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {
                return Err(ConverterError::new(
                    "cross_device_output",
                    "Staging и папка результата находятся на разных томах. Создайте staging рядом с результатом",
                ));
            }
            Err(error) => {
                return Err(ConverterError::new(
                    "publication_failed",
                    format!("Не удалось опубликовать результат: {error}"),
                ));
            }
        }
    }
    Err(ConverterError::new(
        "output_names_exhausted",
        "Не найдено свободное имя результата",
    ))
}
