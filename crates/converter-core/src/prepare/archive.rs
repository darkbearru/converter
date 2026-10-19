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
    .with_message(
        "messages.invalid_archive",
        crate::model::message_args([("error", error.to_string())]),
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
    let password = ctx.archive_passwords.get(&source.id);
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
            )
            .with_message(
                "messages.unsafe_archive_path_3",
                crate::model::message_args([]),
            ));
        }
        if entry.encrypted() && password.is_none() {
            return Err(password_error(source, false));
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
        let mut entry = (if let Some(password) = password {
            archive.by_index_decrypt(index, password.as_bytes())
        } else {
            archive.by_index(index)
        })
        .map_err(|error| match error {
            ZipError::InvalidPassword => password_error(source, true),
            other => zip_error(other),
        })?;
        let encrypted = entry.encrypted();
        if entry.is_dir() {
            continue;
        }
        let name = logical_path(&entry.name().map_err(zip_error)?)?;
        let path = directory.join(format!("file-{index:08}"));
        let mut out = File::options().write(true).create_new(true).open(&path)?;
        let mut size = 0u64;
        loop {
            ctx.control.check()?;
            let count = entry.read(&mut buffer).map_err(|error| {
                if encrypted && error.kind() == std::io::ErrorKind::InvalidData {
                    password_error(source, true)
                } else {
                    error.into()
                }
            })?;
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
            )
            .with_message("messages.invalid_archive_2", crate::model::message_args([])));
        }
        drop(out);
        if is_zip(&path)? || name.to_lowercase().ends_with(".zip") {
            return Err(ConverterError::new(
                "nested_archive",
                "Вложенные ZIP не поддерживаются: извлеките архив отдельно",
            )
            .with_message("messages.nested_archive", crate::model::message_args([])));
        }
        files.push(LogicalFile {
            name,
            path,
            id: format!("{}-{index}", source.id),
            primary,
            local_root: None,
        });
    }
    Ok(files)
}

fn limit() -> ConverterError {
    ConverterError::new(
        "archive_limit",
        "Превышен лимит количества файлов, размера или коэффициента распаковки",
    )
    .with_message("messages.archive_limit_5", crate::model::message_args([]))
}

fn password_error(source: &PreparedFile, invalid: bool) -> ConverterError {
    let (code, message) = if invalid {
        (
            "archive_password_invalid",
            "Пароль ZIP не подошёл или архив повреждён",
        )
    } else {
        ("archive_password_required", "Для ZIP требуется пароль")
    };
    ConverterError::new(code, message).with_message(
        &format!("messages.{code}"),
        crate::model::message_args([
            ("archive_id", source.id.clone()),
            ("name", source.display_name.clone()),
        ]),
    )
}
