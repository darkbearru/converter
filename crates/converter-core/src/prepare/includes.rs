use super::{
    bundle::LogicalFile,
    resource_paths::{folded, logical_path},
};
use crate::{dsl::includes, model::*};
use std::{
    collections::{BTreeSet, VecDeque},
    path::PathBuf,
};

pub(super) fn dependencies(
    files: &mut Vec<LogicalFile>,
    ctx: &RunContext,
    options: &ReadOptions,
) -> Result<Vec<Vec<usize>>> {
    let mut edges = vec![Vec::new(); files.len()];
    let mut queue: VecDeque<_> = files
        .iter()
        .enumerate()
        .filter(|(_, file)| {
            let lower = file.name.to_lowercase();
            lower.ends_with(".dsl") || lower.ends_with(".dsl.dz")
        })
        .map(|(index, _)| index)
        .collect();
    let mut inspected = BTreeSet::new();
    while let Some(index) = queue.pop_front() {
        if !inspected.insert(index) {
            continue;
        }
        ctx.control.check()?;
        let file = &files[index];
        let mut children = Vec::new();
        {
            let prepared = PreparedFile {
                id: file.id.clone(),
                display_name: file.name.clone(),
                path: file.path.clone(),
            };
            for (target, line) in includes::inspect(&prepared, ctx, &options.encoding)? {
                let target = logical_path(&target)
                    .map_err(|_| include_error("unsafe_include_path", &target, &prepared, line))?;
                let parent = prepared
                    .display_name
                    .rsplit_once('/')
                    .map_or("", |(parent, _)| parent);
                let logical = if parent.is_empty() {
                    target.clone()
                } else {
                    format!("{parent}/{target}")
                };
                let child = if let Some(root) = files[index].local_root.clone() {
                    let path = prepared
                        .path
                        .parent()
                        .ok_or_else(|| {
                            include_error("include_not_found", &target, &prepared, line)
                        })?
                        .join(&target);
                    let relative = path.strip_prefix(&root).map_err(|_| {
                        include_error("unsafe_include_path", &target, &prepared, line)
                    })?;
                    let mut checked = root.clone();
                    for component in relative.components() {
                        checked.push(component);
                        let metadata = std::fs::symlink_metadata(&checked).map_err(|_| {
                            include_error("include_not_found", &target, &prepared, line)
                        })?;
                        if metadata.file_type().is_symlink() {
                            return Err(include_error(
                                "unsafe_include_path",
                                &target,
                                &prepared,
                                line,
                            ));
                        }
                    }
                    let canonical = checked.canonicalize()?;
                    if !canonical.is_file() || !canonical.starts_with(&root) {
                        return Err(include_error(
                            "unsafe_include_path",
                            &target,
                            &prepared,
                            line,
                        ));
                    }
                    if let Some(existing) = files
                        .iter()
                        .position(|file| file.local_root.is_some() && file.path == canonical)
                    {
                        existing
                    } else {
                        if files.len() >= ctx.limits.max_archive_entries {
                            return Err(include_error("include_limit", &target, &prepared, line));
                        }
                        let child = files.len();
                        files.push(LogicalFile {
                            name: logical,
                            path: canonical,
                            id: format!("{}-include-{child}", prepared.id),
                            primary: false,
                            local_root: Some(root),
                        });
                        child
                    }
                } else {
                    // У каждого ZIP свой каталог извлечения с непрозрачными именами.
                    // Совпадение имени в другом приложенном архиве не является включением.
                    let mut candidates = files
                        .iter()
                        .enumerate()
                        .filter(|(_, file)| {
                            file.local_root.is_none()
                                && file.path.parent() == prepared.path.parent()
                                && folded(&file.name) == folded(&logical)
                        })
                        .map(|(index, _)| index);
                    let child = candidates.next().ok_or_else(|| {
                        include_error("include_not_found", &target, &prepared, line)
                    })?;
                    if candidates.next().is_some() {
                        return Err(include_error("include_ambiguous", &target, &prepared, line));
                    }
                    child
                };
                queue.push_back(child);
                children.push((child, line));
            }
        }
        edges.resize(files.len(), Vec::new());
        edges[index] = children;
    }
    fn visit(
        index: usize,
        edges: &[Vec<(usize, u64)>],
        files: &[LogicalFile],
        active: &mut BTreeSet<usize>,
        seen: &mut BTreeSet<usize>,
        ordered: &mut Vec<usize>,
        ctx: &RunContext,
    ) -> Result<()> {
        ctx.control.check()?;
        if seen.contains(&index) {
            return Ok(());
        }
        active.insert(index);
        for &(child, line) in &edges[index] {
            let source = PreparedFile {
                id: files[index].id.clone(),
                display_name: files[index].name.clone(),
                path: PathBuf::new(),
            };
            if active.contains(&child) {
                return Err(include_error(
                    "include_cycle",
                    &files[child].name,
                    &source,
                    line,
                ));
            }
            if active.len() >= 64 {
                return Err(include_error(
                    "include_limit",
                    &files[child].name,
                    &source,
                    line,
                ));
            }
            visit(child, edges, files, active, seen, ordered, ctx)?;
        }
        active.remove(&index);
        seen.insert(index);
        ordered.push(index);
        Ok(())
    }
    let mut result = Vec::new();
    for index in 0..files.len() {
        let mut ordered = Vec::new();
        visit(
            index,
            &edges,
            files,
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
            &mut ordered,
            ctx,
        )?;
        ordered.pop();
        result.push(ordered);
    }
    Ok(result)
}

fn include_error(code: &str, target: &str, source: &PreparedFile, line: u64) -> ConverterError {
    let message = match code {
        "unsafe_include_path" => format!("Недопустимый путь #INCLUDE: {target}"),
        "include_cycle" => format!("Циклическое включение DSL: {target}"),
        "include_limit" => "Превышен лимит включённых частей DSL".into(),
        "include_ambiguous" => format!("Найдено несколько частей DSL: {target}"),
        _ => format!("Не найдена часть DSL: {target}"),
    };
    ConverterError::new(code, message)
        .with_message(
            &format!("messages.{code}"),
            message_args([("target", target.to_owned())]),
        )
        .at(&source.display_name, line)
}
