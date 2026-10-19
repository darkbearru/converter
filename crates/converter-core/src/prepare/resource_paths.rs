use crate::model::{ConverterError, InputBundle, ResourceRecord, Result, RunContext};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs::File, io::Read};
use unicode_normalization::UnicodeNormalization;

pub(super) fn logical_path(value: &str) -> Result<String> {
    let value = value.replace('\\', "/");
    if value.is_empty()
        || value.starts_with('/')
        || value.contains(['\0', ':'])
        || value.split('/').any(|part| matches!(part, ".." | "." | ""))
    {
        return Err(ConverterError::new(
            "unsafe_archive_path",
            format!("Недопустимый относительный путь: {value}"),
        )
        .with_message(
            "messages.unsafe_archive_path_2",
            crate::model::message_args([("value", value.to_string())]),
        ));
    }
    Ok(value)
}

pub(super) fn folded(key: &str) -> String {
    key.to_lowercase().nfc().collect()
}

pub(super) fn resource_key(path: &str) -> String {
    format!("\\{}", path.replace('/', "\\"))
}

pub(crate) fn unique_resources(
    resources: Vec<ResourceRecord>,
    ctx: &RunContext,
) -> Result<Vec<ResourceRecord>> {
    let mut unique = BTreeMap::<String, ResourceRecord>::new();
    for resource in resources {
        ctx.control.check()?;
        let key = folded(&resource.key);
        if let Some(previous) = unique.get(&key) {
            if checksum(previous, ctx)? != checksum(&resource, ctx)? {
                return Err(ConverterError::new(
                    "ambiguous_resource",
                    format!(
                        "Разные ресурсы имеют совпадающие имена: {} / {}",
                        previous.key, resource.key
                    ),
                )
                .with_message(
                    "messages.ambiguous_resource",
                    crate::model::message_args([
                        ("arg0", previous.key.to_string()),
                        ("arg1", resource.key.to_string()),
                    ]),
                ));
            }
        } else {
            unique.insert(key, resource);
        }
    }
    Ok(unique.into_values().collect())
}

fn checksum(resource: &ResourceRecord, ctx: &RunContext) -> Result<[u8; 32]> {
    let mut hash = Sha256::new();
    let mut file = File::open(&resource.path)?;
    let mut buffer = vec![0; ctx.limits.resource_buffer_bytes.clamp(1, 1024 * 1024)];
    loop {
        ctx.control.check()?;
        let size = file.read(&mut buffer)?;
        if size == 0 {
            return Ok(hash.finalize().into());
        }
        hash.update(&buffer[..size]);
    }
}

pub fn resolve_resource(bundle: &InputBundle, logical_key: &str) -> Result<Option<ResourceRecord>> {
    // Внутренние MDD-ключи начинаются одним обратным слешем; UNC запрещён.
    let relative = if logical_key.starts_with("\\\\") {
        logical_key
    } else {
        logical_key.strip_prefix('\\').unwrap_or(logical_key)
    };
    let key = resource_key(&logical_path(relative)?);
    if let Some(exact) = bundle.resources.iter().find(|r| r.key == key) {
        return Ok(Some(exact.clone()));
    }
    let search = folded(&key);
    let mut candidates = bundle.resources.iter().filter(|r| folded(&r.key) == search);
    let first = candidates.next().cloned();
    if candidates.next().is_some() {
        return Err(ConverterError::new(
            "ambiguous_resource",
            format!("Неоднозначное имя ресурса: {logical_key}"),
        )
        .with_message(
            "messages.ambiguous_resource_2",
            crate::model::message_args([("logical_key", logical_key.to_string())]),
        ));
    }
    // Вызывающий renderer сравнивает возвращённый key с запросом и пишет предупреждение.
    Ok(first)
}
