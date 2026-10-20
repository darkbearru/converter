use super::{bundle::LogicalFile, resource_paths::folded};
use crate::model::*;
use std::io::ErrorKind;

fn annotation_name(name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    let suffix = if lower.ends_with(".dsl.dz") {
        7
    } else if lower.ends_with(".dsl") {
        4
    } else {
        return None;
    };
    Some(format!("{}.ann", &name[..name.len() - suffix]))
}

pub(super) fn discover(
    files: &mut Vec<LogicalFile>,
    ctx: &RunContext,
) -> Result<Vec<Option<usize>>> {
    let mut annotations = vec![None; files.len()];
    for index in 0..annotations.len() {
        ctx.control.check()?;
        let Some(name) = annotation_name(&files[index].name) else {
            continue;
        };
        annotations[index] = if let Some(root) = &files[index].local_root {
            // Имя соседнего ANN выводится из физического имени DSL, а не из его содержимого.
            let path = &files[index].path;
            let Some(sibling) = path
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(annotation_name)
            else {
                continue;
            };
            let candidate = path.with_file_name(sibling);
            let metadata = match std::fs::symlink_metadata(&candidate) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(super::bundle::unsafe_link());
            }
            let canonical = candidate.canonicalize()?;
            if !canonical.starts_with(root) || canonical.parent() != path.parent() {
                return Err(super::bundle::unsafe_link());
            }
            if let Some(existing) = files
                .iter()
                .position(|file| file.local_root.is_some() && file.path == canonical)
            {
                Some(existing)
            } else {
                if files.len() >= ctx.limits.max_archive_entries {
                    return Err(ConverterError::new(
                        "archive_limit",
                        "Слишком много файлов источников",
                    )
                    .with_message("messages.archive_limit", message_args([])));
                }
                let annotation = files.len();
                files.push(LogicalFile {
                    name,
                    path: canonical,
                    id: format!("{}-annotation", files[index].id),
                    primary: false,
                    local_root: Some(root.clone()),
                });
                Some(annotation)
            }
        } else {
            let mut candidates = files.iter().enumerate().filter(|(_, file)| {
                file.local_root.is_none()
                    && file.path.parent() == files[index].path.parent()
                    && folded(&file.name) == folded(&name)
            });
            let annotation = candidates.next().map(|(index, _)| index);
            if let Some((_, other)) = candidates.next() {
                return Err(ConverterError::new(
                    "ambiguous_resource",
                    format!("Найдено несколько аннотаций: {name}"),
                )
                .with_message(
                    "messages.ambiguous_resource",
                    message_args([("arg0", name), ("arg1", other.name.clone())]),
                ));
            }
            annotation
        };
    }
    Ok(annotations)
}
