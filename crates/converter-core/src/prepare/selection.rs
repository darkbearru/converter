use crate::{formats::FormatRegistry, model::*};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs::File,
    io::Read,
};
use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Debug)]
pub struct PrimaryDictionarySelection {
    pub main_id: String,
    pub main_title: String,
    pub independent_ids: Vec<String>,
}

fn name(value: &str) -> String {
    value.trim().nfc().collect::<String>().to_lowercase()
}

pub fn select_primary_dictionary(
    bundles: &[InputBundle],
    options: &ReadOptions,
    ctx: &RunContext,
) -> Result<Option<PrimaryDictionarySelection>> {
    select_primary_dictionary_with_registry(bundles, options, ctx, &FormatRegistry::builtins())
}

pub fn select_primary_dictionary_with_registry(
    bundles: &[InputBundle],
    options: &ReadOptions,
    ctx: &RunContext,
    registry: &FormatRegistry,
) -> Result<Option<PrimaryDictionarySelection>> {
    ctx.control.check()?;
    if bundles.len() < 2 {
        return Ok(None);
    }
    let mut relationships = Vec::new();
    for bundle in bundles {
        ctx.control.check()?;
        let analysis = (|| {
            let mut prefix = Vec::new();
            File::open(&bundle.main.path)?
                .take(64 * 1024)
                .read_to_end(&mut prefix)?;
            let format = registry.detect(&prefix, &bundle.main.display_name)?;
            registry.get(&format)?.relationships(bundle, options, ctx)
        })();
        match analysis {
            Ok(Some(value)) => relationships.push(value),
            Ok(None) => return Ok(None),
            Err(error) => {
                ctx.control.check()?;
                if error.code == "cancelled" {
                    return Err(error);
                }
                return Ok(None);
            }
        }
    }
    let mut names = BTreeMap::new();
    for (index, relationship) in relationships.iter().enumerate() {
        let key = name(&relationship.title);
        if key.is_empty() || names.insert(key, index).is_some() {
            return Ok(None);
        }
    }
    let mut edges = vec![BTreeSet::new(); bundles.len()];
    let mut incoming = vec![0usize; bundles.len()];
    for (index, relationship) in relationships.iter().enumerate() {
        for target in &relationship.external_dictionaries {
            let Some(&target) = names.get(&name(target)) else {
                return Ok(None);
            };
            if target != index && edges[index].insert(target) {
                incoming[target] += 1;
            }
        }
    }
    let candidates: Vec<_> = (0..bundles.len())
        .filter(|&index| incoming[index] > 0 && edges[index].is_empty())
        .collect();
    if candidates.len() != 1 {
        return Ok(None);
    }
    let mut remaining = incoming.clone();
    let mut queue: VecDeque<_> = (0..bundles.len())
        .filter(|&index| remaining[index] == 0)
        .collect();
    let mut visited = 0;
    while let Some(index) = queue.pop_front() {
        ctx.control.check()?;
        visited += 1;
        for &target in &edges[index] {
            remaining[target] -= 1;
            if remaining[target] == 0 {
                queue.push_back(target);
            }
        }
    }
    if visited != bundles.len() {
        return Ok(None);
    }
    let main = candidates[0];
    // Ссылка из большого словаря может вести в небольшой справочник, а не в
    // головной словарь. В таком случае сохраняем явный выбор пользователя.
    if (0..bundles.len()).any(|index| {
        (incoming[index] > 0 || !edges[index].is_empty())
            && relationships[index].source_articles > relationships[main].source_articles
    }) {
        return Ok(None);
    }
    Ok(Some(PrimaryDictionarySelection {
        main_id: bundles[main].main.id.clone(),
        main_title: relationships[main].title.clone(),
        independent_ids: (0..bundles.len())
            .filter(|&index| incoming[index] == 0 && edges[index].is_empty())
            .map(|index| bundles[index].main.id.clone())
            .collect(),
    }))
}
