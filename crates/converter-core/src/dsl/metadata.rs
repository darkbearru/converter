use super::{
    annotations::description,
    decoder::{ResolvedEncoding, decode_lines, open_source},
    lexer::Lexer,
};
use crate::model::*;
use encoding_rs::{Encoding, UTF_8, UTF_16BE, UTF_16LE, WINDOWS_1251};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{Cursor, Read},
};

pub(super) fn directive(text: &str, meta: &mut DictionaryMetadata) -> Result<bool> {
    let (name, value) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let target = match name {
        "#NAME" => &mut meta.title,
        "#INDEX_LANGUAGE" => &mut meta.index_language,
        "#CONTENTS_LANGUAGE" => &mut meta.contents_language,
        "#SOURCE_CODE_PAGE" => {
            // Декларация уже учтена декодером; Unicode BOM и ручной выбор важнее неё.
            return Ok(true);
        }
        _ => return Ok(false),
    };
    let value = value.trim();
    if !value.starts_with('"') || !value.ends_with('"') || value.len() < 2 {
        return Err(ConverterError::new(
            "invalid_dsl",
            format!("Директива {name} должна содержать значение в кавычках"),
        )
        .with_message(
            "messages.invalid_dsl",
            crate::model::message_args([("name", name.to_string())]),
        ));
    }
    *target = value[1..value.len() - 1]
        .replace("\\\"", "\"")
        .replace("\\\\", "\\");
    Ok(true)
}

pub(super) fn inspect_metadata(
    bundle: &InputBundle,
    encoding: &ResolvedEncoding,
    ctx: &RunContext,
) -> Result<DictionaryMetadata> {
    let mut meta = inspect_header(&bundle.main, encoding, ctx)?;
    let merged = merged_title(bundle, &meta.title, encoding, ctx)?;
    if let Some(title) = &merged {
        meta.title.clone_from(title);
    }
    let annotation = if let Some(file) = &bundle.annotation {
        let owner = bundle.includes.iter().find(|part| {
            let name = part.display_name.to_lowercase();
            let name = name.strip_suffix(".dz").unwrap_or(&name);
            part.path.parent() == file.path.parent()
                && name
                    .strip_suffix(".dsl")
                    .is_some_and(|stem| format!("{stem}.ann") == file.display_name.to_lowercase())
        });
        let annotation_encoding = if let Some(owner) = owner {
            super::includes::resolve_part_encoding(
                owner,
                &EncodingChoice::Manual(encoding.encoding_name.clone()),
                ctx,
            )?
        } else {
            encoding.clone()
        };
        read_annotation(file, &annotation_encoding, ctx)?
    } else {
        String::new()
    };
    meta.description = description(&annotation, merged.is_some());
    Ok(meta)
}

pub(super) fn dictionary_names(
    bundle: &InputBundle,
    options: &ReadOptions,
    ctx: &RunContext,
) -> Result<Vec<String>> {
    let encoding = super::decoder::resolve_encoding(&bundle.main, options.encoding.clone(), ctx)?;
    let main = inspect_header(&bundle.main, &encoding, ctx)?;
    let mut names = vec![main.title.clone()];
    for file in &bundle.includes {
        let part_encoding = super::includes::resolve_part_encoding(
            file,
            &EncodingChoice::Manual(encoding.encoding_name.clone()),
            ctx,
        )?;
        let title = inspect_header(file, &part_encoding, ctx)?.title;
        if !names.contains(&title) {
            names.push(title);
        }
    }
    if let Some(title) = merged_title(bundle, &main.title, &encoding, ctx)?
        && !names.contains(&title)
    {
        names.push(title);
    }
    Ok(names)
}

fn inspect_header(
    file: &PreparedFile,
    encoding: &ResolvedEncoding,
    ctx: &RunContext,
) -> Result<DictionaryMetadata> {
    let mut meta = DictionaryMetadata {
        source_encoding: encoding.encoding_name.clone(),
        ..DictionaryMetadata::default()
    };
    let mut lexer = Lexer::default();
    decode_lines(open_source(file)?, encoding.encoding, ctx, |line| {
        let text = lexer.line(&line.text);
        if text.trim().is_empty() {
            return Ok(true);
        }
        if text.starts_with('#') {
            directive(&text, &mut meta)
                .map_err(|error| error.at(&file.display_name, line.number))?;
            Ok(true)
        } else {
            Ok(false)
        }
    })?;
    if meta.title.is_empty() {
        meta.title.clone_from(&file.display_name);
    }
    Ok(meta)
}

fn part_title(title: &str) -> Option<(&str, usize, usize)> {
    let (base, suffix) = title.trim_end().strip_suffix(')')?.rsplit_once('(')?;
    let (part, total) = suffix.split_once('/')?;
    let part = part.trim().parse::<usize>().ok()?;
    let total = total.trim().parse::<usize>().ok()?;
    let base = base.trim_end();
    (part > 0 && part <= total && total > 1 && !base.is_empty()).then_some((base, part, total))
}

fn merged_title(
    bundle: &InputBundle,
    title: &str,
    encoding: &ResolvedEncoding,
    ctx: &RunContext,
) -> Result<Option<String>> {
    let Some((base, part, total)) = part_title(title) else {
        return Ok(None);
    };
    if bundle.includes.len() + 1 != total {
        return Ok(None);
    }
    let mut parts = BTreeSet::from([part]);
    for file in &bundle.includes {
        let encoding = super::includes::resolve_part_encoding(
            file,
            &EncodingChoice::Manual(encoding.encoding_name.clone()),
            ctx,
        )?;
        let metadata = inspect_header(file, &encoding, ctx)?;
        let Some((part_base, part, part_total)) = part_title(&metadata.title) else {
            return Ok(None);
        };
        if part_base != base || part_total != total || !parts.insert(part) {
            return Ok(None);
        }
    }
    Ok(Some(base.to_owned()))
}

fn annotation_limit(file: &PreparedFile) -> ConverterError {
    ConverterError::new("article_too_large", "Аннотация превышает допустимый размер")
        .with_message("messages.article_too_large_3", message_args([]))
        .at(&file.display_name, 1)
}

fn read_annotation(
    file: &PreparedFile,
    fallback: &ResolvedEncoding,
    ctx: &RunContext,
) -> Result<String> {
    ctx.control.check()?;
    let mut bytes = Vec::new();
    File::open(&file.path)?
        .take((ctx.limits.max_article_bytes as u64).saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > ctx.limits.max_article_bytes {
        return Err(annotation_limit(file));
    }
    let encoding = if let Some((encoding, _)) = Encoding::for_bom(&bytes) {
        encoding
    } else if bytes.contains(&0) && [UTF_16LE, UTF_16BE].contains(&fallback.encoding) {
        fallback.encoding
    } else if std::str::from_utf8(&bytes).is_ok() {
        UTF_8
    } else if [UTF_8, UTF_16LE, UTF_16BE].contains(&fallback.encoding) {
        WINDOWS_1251
    } else {
        fallback.encoding
    };
    let mut text = String::new();
    let mut first = true;
    decode_lines(Cursor::new(bytes), encoding, ctx, |line| {
        if text
            .len()
            .saturating_add(line.text.len())
            .saturating_add(usize::from(!first))
            > ctx.limits.max_article_bytes
        {
            return Err(annotation_limit(file));
        }
        if !first {
            text.push('\n');
        }
        first = false;
        text.push_str(&line.text);
        Ok(true)
    })?;
    Ok(text)
}
