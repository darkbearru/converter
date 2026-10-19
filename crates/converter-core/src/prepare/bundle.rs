use super::{
    archive::{extract, is_zip},
    resource_paths::{logical_path, resource_key, unique_resources},
};
use crate::{formats::FormatRegistry, model::*};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

pub(super) struct LogicalFile {
    pub name: String,
    pub path: PathBuf,
    pub id: String,
    pub primary: bool,
    pub local_root: Option<PathBuf>,
}

pub fn prepare_source(
    main: &PreparedFile,
    attachments: &[PreparedFile],
    ctx: &RunContext,
) -> Result<Vec<InputBundle>> {
    prepare_source_with_registry(main, attachments, ctx, &FormatRegistry::builtins())
}

pub fn prepare_source_with_registry(
    main: &PreparedFile,
    attachments: &[PreparedFile],
    ctx: &RunContext,
    registry: &FormatRegistry,
) -> Result<Vec<InputBundle>> {
    prepare_source_with_registry_and_options(
        main,
        attachments,
        ctx,
        registry,
        &ReadOptions::default(),
    )
}

pub fn prepare_source_with_options(
    main: &PreparedFile,
    attachments: &[PreparedFile],
    ctx: &RunContext,
    options: &ReadOptions,
) -> Result<Vec<InputBundle>> {
    prepare_source_with_registry_and_options(
        main,
        attachments,
        ctx,
        &FormatRegistry::builtins(),
        options,
    )
}

fn prepare_source_with_registry_and_options(
    main: &PreparedFile,
    attachments: &[PreparedFile],
    ctx: &RunContext,
    registry: &FormatRegistry,
    options: &ReadOptions,
) -> Result<Vec<InputBundle>> {
    ctx.control.check()?;
    if ctx.limits.resource_buffer_bytes == 0 {
        return Err(ConverterError::new(
            "invalid_limits",
            "Буфер копирования не может быть пустым",
        )
        .with_message("messages.invalid_limits", crate::model::message_args([])));
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
                )
                .with_message("messages.invalid_source", crate::model::message_args([])));
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
                path: source.path.canonicalize()?,
                id: source.id.clone(),
                primary: index == 0,
                local_root: source.path.canonicalize()?.parent().map(Path::to_path_buf),
            });
        }
    }
    let dependencies = super::includes::dependencies(&mut files, ctx, options)?;
    let included: std::collections::BTreeSet<usize> =
        dependencies.iter().flatten().copied().collect();
    if files.len() > ctx.limits.max_archive_entries {
        return Err(
            ConverterError::new("archive_limit", "Слишком много файлов источников")
                .with_message("messages.archive_limit", crate::model::message_args([])),
        );
    }
    let mut size = 0u64;
    for file in &files {
        ctx.control.check()?;
        size = size
            .checked_add(std::fs::metadata(&file.path)?.len())
            .ok_or_else(|| {
                ConverterError::new("archive_limit", "Переполнение размера источников")
                    .with_message("messages.archive_limit_2", crate::model::message_args([]))
            })?;
        if size > ctx.limits.max_unpacked_bytes {
            return Err(ConverterError::new(
                "archive_limit",
                "Источники превышают допустимый размер",
            )
            .with_message("messages.archive_limit_3", crate::model::message_args([])));
        }
    }
    let descriptors = registry.descriptors();
    let mut formats = Vec::with_capacity(files.len());
    for file in &files {
        ctx.control.check()?;
        let lower = file.name.to_lowercase();
        let candidate = file.id == main.id
            || descriptors.iter().any(|format| {
                format
                    .extensions
                    .iter()
                    .any(|ext| lower.ends_with(&format!(".{}", ext.to_lowercase())))
            });
        formats.push(if candidate {
            let mut prefix = Vec::new();
            File::open(&file.path)?
                .take(64 * 1024)
                .read_to_end(&mut prefix)?;
            Some(registry.detect(&prefix, &file.name)?)
        } else {
            None
        });
    }
    let mut mains = Vec::new();
    for (index, file) in files.iter().enumerate() {
        if file.primary
            && !included.contains(&index)
            && let Some(format) = &formats[index]
            && !registry.get(format)?.is_auxiliary(&file.name)
        {
            mains.push(index);
        }
    }
    if mains.is_empty() {
        return Err(ConverterError::new(
            "dictionary_not_found",
            "В источнике не найден поддерживаемый словарь",
        )
        .with_message(
            "messages.dictionary_not_found",
            crate::model::message_args([]),
        ));
    }
    let mut bundles = Vec::new();
    for index in &mains {
        let source = &files[*index];
        let reader = registry.get(formats[*index].as_deref().ok_or_else(|| {
            ConverterError::new("unsupported_format", "Не определён формат").with_message(
                "messages.unsupported_format_3",
                crate::model::message_args([]),
            )
        })?)?;
        let directory = source
            .name
            .rsplit_once('/')
            .map_or("", |(parent, _)| parent);
        let prefix = if directory.is_empty() {
            String::new()
        } else {
            format!("{directory}/")
        };
        let mut abbreviations = files
            .iter()
            .enumerate()
            .filter(|(candidate, file)| {
                formats[*candidate] == formats[*index]
                    && reader.is_companion(&source.name, &file.name)
            })
            .map(|(_, file)| file);
        let abbreviation = abbreviations.next();
        if abbreviations.next().is_some() {
            return Err(ConverterError::new(
                "ambiguous_abbreviations",
                "Найдено несколько словарей сокращений с одним именем",
            )
            .with_message(
                "messages.ambiguous_abbreviations",
                crate::model::message_args([]),
            ));
        }
        let resources = files
            .iter()
            .enumerate()
            .filter(|(index, _)| formats[*index].is_none() && !included.contains(index))
            .filter_map(|(_, file)| {
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
            includes: dependencies[*index]
                .iter()
                .map(|part| prepared(&files[*part]))
                .collect(),
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
    .with_message(
        "messages.unsafe_archive_path",
        crate::model::message_args([]),
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
                )
                .with_message("messages.invalid_source_2", crate::model::message_args([])));
            }
            let relative = path.strip_prefix(&root).map_err(|_| unsafe_link())?;
            let name = relative_name(relative)?;
            files.push(LogicalFile {
                name,
                path,
                id: format!("{}-{}", source.id, files.len()),
                primary: false,
                local_root: Some(root.clone()),
            });
            if files.len() > ctx.limits.max_archive_entries {
                return Err(
                    ConverterError::new("archive_limit", "Слишком много файлов ресурсов")
                        .with_message("messages.archive_limit_4", crate::model::message_args([])),
                );
            }
        }
    }
    Ok(())
}

fn relative_name(path: &Path) -> Result<String> {
    let name = path.to_str().ok_or_else(|| {
        ConverterError::new("invalid_resource_name", "Имя ресурса не является Unicode")
            .with_message(
                "messages.invalid_resource_name",
                crate::model::message_args([]),
            )
    })?;
    logical_path(name)
}
