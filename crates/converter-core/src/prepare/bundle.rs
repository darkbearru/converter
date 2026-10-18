use super::{
    archive::{extract, is_zip},
    resource_paths::{logical_path, resource_key, unique_resources},
};
use crate::model::*;
use std::path::{Path, PathBuf};

pub(super) struct LogicalFile {
    pub name: String,
    pub path: PathBuf,
    pub id: String,
    pub primary: bool,
}

fn is_dsl(name: &str) -> bool {
    let name = name.to_lowercase();
    name.ends_with(".dsl") || name.ends_with(".dsl.dz")
}
fn stem(name: &str) -> &str {
    name.strip_suffix(".dz")
        .unwrap_or(name)
        .strip_suffix(".dsl")
        .unwrap_or(name)
}
fn is_abbreviation(name: &str) -> bool {
    stem(&name.to_lowercase()).ends_with("_abrv")
}

pub fn prepare_source(
    main: &PreparedFile,
    attachments: &[PreparedFile],
    ctx: &RunContext,
) -> Result<Vec<InputBundle>> {
    ctx.control.check()?;
    if ctx.limits.resource_buffer_bytes == 0 {
        return Err(ConverterError::new(
            "invalid_limits",
            "Буфер копирования не может быть пустым",
        ));
    }
    std::fs::create_dir_all(&ctx.scratch_dir)?;
    let working = tempfile::Builder::new()
        .prefix("prepared-")
        .tempdir_in(&ctx.scratch_dir)?;
    let mut files = Vec::new();
    for (index, source) in std::iter::once(main).chain(attachments).enumerate() {
        ctx.control.check()?;
        let metadata = std::fs::symlink_metadata(&source.path)?;
        if metadata.file_type().is_symlink() {
            return Err(unsafe_link());
        }
        if metadata.is_dir() {
            if index == 0 {
                return Err(ConverterError::new(
                    "invalid_source",
                    "Основной источник должен быть файлом DSL или ZIP",
                ));
            }
            gather_folder(source, &mut files, ctx)?;
        } else if is_zip(&source.path)? {
            files.extend(extract(
                source,
                &working.path().join(format!("archive-{index}")),
                index == 0,
                ctx,
            )?);
        } else {
            files.push(LogicalFile {
                name: logical_path(&source.display_name)?,
                path: source.path.clone(),
                id: source.id.clone(),
                primary: index == 0,
            });
        }
    }
    if files.len() > ctx.limits.max_archive_entries {
        return Err(ConverterError::new(
            "archive_limit",
            "Слишком много файлов источников",
        ));
    }
    let mut size = 0u64;
    for file in &files {
        ctx.control.check()?;
        size = size
            .checked_add(std::fs::metadata(&file.path)?.len())
            .ok_or_else(|| {
                ConverterError::new("archive_limit", "Переполнение размера источников")
            })?;
        if size > ctx.limits.max_unpacked_bytes {
            return Err(ConverterError::new(
                "archive_limit",
                "Источники превышают допустимый размер",
            ));
        }
    }
    let mains: Vec<_> = files
        .iter()
        .filter(|file| {
            file.primary
                && !is_abbreviation(&file.name)
                && (is_dsl(&file.name) || file.id == main.id)
        })
        .collect();
    if mains.is_empty() {
        return Err(ConverterError::new(
            "dictionary_not_found",
            "В источнике не найден основной DSL-словарь",
        ));
    }
    let mut bundles = Vec::new();
    for source in &mains {
        let directory = source
            .name
            .rsplit_once('/')
            .map_or("", |(parent, _)| parent);
        let prefix = if directory.is_empty() {
            String::new()
        } else {
            format!("{directory}/")
        };
        let abbreviation_name = format!("{}_abrv", stem(&source.name.to_lowercase()));
        let mut abbreviations = files.iter().filter(|file| {
            is_dsl(&file.name) && stem(&file.name.to_lowercase()) == abbreviation_name
        });
        let abbreviation = abbreviations.next();
        if abbreviations.next().is_some() {
            return Err(ConverterError::new(
                "ambiguous_abbreviations",
                "Найдено несколько словарей сокращений с одним именем",
            ));
        }
        let resources = files
            .iter()
            .filter(|file| {
                !is_dsl(&file.name) && !mains.iter().any(|main| std::ptr::eq(*main, *file))
            })
            .filter_map(|file| {
                let name = if file.primary {
                    file.name.strip_prefix(&prefix)?
                } else {
                    file.name.as_str()
                };
                Some(ResourceRecord {
                    key: resource_key(name),
                    path: file.path.clone(),
                })
            })
            .collect();
        bundles.push(InputBundle {
            main: prepared(source),
            abbreviations: abbreviation.map(prepared),
            resources: unique_resources(resources, ctx)?,
        });
    }
    // Данные принадлежат рабочему каталогу задания и живут до его явной очистки.
    let _ = working.keep();
    Ok(bundles)
}

fn prepared(file: &LogicalFile) -> PreparedFile {
    PreparedFile {
        id: file.id.clone(),
        display_name: file.name.clone(),
        path: file.path.clone(),
    }
}

fn unsafe_link() -> ConverterError {
    ConverterError::new(
        "unsafe_archive_path",
        "Символические ссылки в источниках ресурсов не поддерживаются",
    )
}

fn gather_folder(
    source: &PreparedFile,
    files: &mut Vec<LogicalFile>,
    ctx: &RunContext,
) -> Result<()> {
    let root = std::fs::canonicalize(&source.path)?;
    let mut pending = vec![root.clone()];
    while let Some(directory) = pending.pop() {
        ctx.control.check()?;
        for entry in std::fs::read_dir(directory)? {
            ctx.control.check()?;
            let entry = entry?;
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(unsafe_link());
            }
            if metadata.is_dir() {
                pending.push(path);
                continue;
            }
            if !metadata.is_file() {
                return Err(ConverterError::new(
                    "invalid_source",
                    "Ресурс не является обычным файлом",
                ));
            }
            let relative = path.strip_prefix(&root).map_err(|_| unsafe_link())?;
            let name = relative_name(relative)?;
            files.push(LogicalFile {
                name,
                path,
                id: format!("{}-{}", source.id, files.len()),
                primary: false,
            });
            if files.len() > ctx.limits.max_archive_entries {
                return Err(ConverterError::new(
                    "archive_limit",
                    "Слишком много файлов ресурсов",
                ));
            }
        }
    }
    Ok(())
}

fn relative_name(path: &Path) -> Result<String> {
    let name = path.to_str().ok_or_else(|| {
        ConverterError::new("invalid_resource_name", "Имя ресурса не является Unicode")
    })?;
    logical_path(name)
}
