use super::{bundle::LogicalFile, resource_paths::logical_path};
use crate::model::{ConverterError, PreparedFile, Result, RunContext};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};
use zip::{ZipArchive, result::ZipError};

fn zip_error(error: ZipError) -> ConverterError {
    ConverterError::new(
        "invalid_archive",
        format!("Не удалось прочитать ZIP: {error}"),
    )
}

pub(super) fn is_zip(path: &Path) -> Result<bool> {
    let mut prefix = [0; 4];
    let size = File::open(path)?.read(&mut prefix)?;
    Ok(size == 4 && matches!(&prefix, b"PK\x03\x04" | b"PK\x05\x06" | b"PK\x07\x08"))
}

pub(super) fn extract(
    source: &PreparedFile,
    directory: &Path,
    primary: bool,
    ctx: &RunContext,
) -> Result<Vec<LogicalFile>> {
    let mut archive = ZipArchive::new(File::open(&source.path)?).map_err(zip_error)?;
    if archive.len() > ctx.limits.max_archive_entries {
        return Err(limit());
    }
    let mut declared = 0u64;
    // Проверяем каталог до извлечения: имена никогда не становятся физическими путями.
    for index in 0..archive.len() {
        ctx.control.check()?;
        let entry = archive.by_index_data(index).map_err(zip_error)?;
        logical_path(
            entry
                .name()
                .map_err(zip_error)?
                .trim_end_matches(['/', '\\']),
        )?;
        if entry.is_symlink() {
            return Err(ConverterError::new(
                "unsafe_archive_path",
                "Символические ссылки в ZIP не поддерживаются",
            ));
        }
        if entry.encrypted() {
            return Err(ConverterError::new(
                "encrypted_archive",
                "Зашифрованные ZIP не поддерживаются",
            ));
        }
        declared = declared.checked_add(entry.size()).ok_or_else(limit)?;
        if declared > ctx.limits.max_unpacked_bytes
            || entry.size()
                > entry
                    .compressed_size()
                    .max(1)
                    .saturating_mul(ctx.limits.max_compression_ratio)
        {
            return Err(limit());
        }
    }
    std::fs::create_dir_all(directory)?;
    let mut files = Vec::new();
    let mut unpacked = 0u64;
    let mut buffer = vec![0; ctx.limits.resource_buffer_bytes.clamp(1, 1024 * 1024)];
    for index in 0..archive.len() {
        ctx.control.check()?;
        let mut entry = archive.by_index(index).map_err(zip_error)?;
        if entry.is_dir() {
            continue;
        }
        let name = logical_path(&entry.name().map_err(zip_error)?)?;
        let path = directory.join(format!("file-{index:08}"));
        let mut out = File::options().write(true).create_new(true).open(&path)?;
        let mut size = 0u64;
        loop {
            ctx.control.check()?;
            let count = entry.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            size = size.checked_add(count as u64).ok_or_else(limit)?;
            unpacked = unpacked.checked_add(count as u64).ok_or_else(limit)?;
            if size > entry.size() || unpacked > ctx.limits.max_unpacked_bytes {
                return Err(limit());
            }
            out.write_all(&buffer[..count])?;
        }
        if size != entry.size() {
            return Err(ConverterError::new(
                "invalid_archive",
                "Размер файла ZIP не совпал с каталогом",
            ));
        }
        drop(out);
        if is_zip(&path)? || name.to_lowercase().ends_with(".zip") {
            return Err(ConverterError::new(
                "nested_archive",
                "Вложенные ZIP не поддерживаются: извлеките архив отдельно",
            ));
        }
        files.push(LogicalFile {
            name,
            path,
            id: format!("{}-{index}", source.id),
            primary,
        });
    }
    Ok(files)
}

fn limit() -> ConverterError {
    ConverterError::new(
        "archive_limit",
        "Превышен лимит количества файлов, размера или коэффициента распаковки",
    )
}
