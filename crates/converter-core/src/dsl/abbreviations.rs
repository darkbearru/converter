use super::{
    decoder::{ResolvedEncoding, resolve_encoding},
    headwords::expand_headwords,
    markup,
    parser::{ArticleAst, AstSink, parse_raw},
};
use crate::model::*;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct AbbreviationCache {
    values: HashMap<String, Option<String>>,
    bytes: usize,
}

impl AbbreviationCache {
    pub fn resolve(
        &mut self,
        bundle: &InputBundle,
        options: &ReadOptions,
        wanted: &HashSet<String>,
        ctx: &RunContext,
    ) -> Result<HashMap<String, String>> {
        let missing = wanted
            .iter()
            .filter(|key| !self.values.contains_key(*key))
            .cloned()
            .collect();
        let mut values = lookup(bundle, options, &missing, ctx)?;
        for key in wanted {
            if let Some(Some(value)) = self.values.get(key) {
                values.insert(key.clone(), value.clone());
            }
        }
        let total = values.iter().fold(0usize, |size, (key, value)| {
            size.saturating_add(key.len())
                .saturating_add(value.len())
                .saturating_add(128)
        });
        if total > ctx.limits.max_html_bytes {
            return Err(ConverterError::new(
                "article_too_large",
                "Сокращения статьи превышают допустимый размер HTML",
            )
            .with_message(
                "messages.article_too_large_2",
                crate::model::message_args([]),
            ));
        }
        let limit = ctx.limits.sort_buffer_bytes.min(16 * 1024 * 1024);
        for key in wanted {
            if self.values.contains_key(key) {
                continue;
            }
            let value = values.get(key).cloned();
            let cost = key
                .len()
                .saturating_add(value.as_ref().map_or(0, String::len))
                .saturating_add(128);
            if cost > limit {
                continue;
            }
            if self.bytes.saturating_add(cost) > limit {
                self.values.clear();
                self.bytes = 0;
            }
            self.values.insert(key.clone(), value);
            self.bytes += cost;
        }
        Ok(values)
    }
}

pub(super) fn lookup(
    bundle: &InputBundle,
    options: &ReadOptions,
    wanted: &HashSet<String>,
    ctx: &RunContext,
) -> Result<HashMap<String, String>> {
    let Some(source) = &bundle.abbreviations else {
        return Ok(HashMap::new());
    };
    if wanted.is_empty() {
        return Ok(HashMap::new());
    }
    let mut quiet = ctx.clone();
    let progress = ctx.progress.clone();
    quiet.progress = std::sync::Arc::new(move |event| {
        if matches!(event, CoreEvent::Diagnostic(_)) {
            progress(event);
        }
    });
    let ctx = &quiet;
    let input = InputBundle {
        includes: vec![],
        main: source.clone(),
        abbreviations: None,
        resources: Vec::new(),
    };
    let encoding: ResolvedEncoding = match resolve_encoding(source, EncodingChoice::Auto, ctx) {
        Err(error) if error.code == "encoding_required" => {
            resolve_encoding(source, options.encoding.clone(), ctx)?
        }
        result => result?,
    };
    let mut sink = AbbreviationSink {
        wanted,
        values: HashMap::new(),
        ctx,
        bytes: 0,
    };
    parse_raw(&input, &encoding, &mut sink, ctx)?;
    Ok(sink.values)
}

struct AbbreviationSink<'a> {
    wanted: &'a HashSet<String>,
    values: HashMap<String, String>,
    ctx: &'a RunContext,
    bytes: usize,
}
impl AstSink for AbbreviationSink<'_> {
    fn push(&mut self, article: ArticleAst) -> Result<()> {
        for raw in &article.headwords {
            for key in expand_headwords(raw, self.ctx.limits.max_headword_variants)? {
                if self.wanted.contains(&key) {
                    let (nodes, _) = markup::parse(&article.body, &key, self.ctx)?;
                    let value = markup::plain(&nodes).trim().to_owned();
                    if let Some(previous) = self.values.get(&key) {
                        self.bytes = self.bytes.saturating_sub(previous.len() + key.len() + 128);
                    }
                    self.bytes = self
                        .bytes
                        .saturating_add(value.len())
                        .saturating_add(key.len())
                        .saturating_add(128);
                    if self.bytes > self.ctx.limits.max_html_bytes {
                        return Err(ConverterError::new(
                            "article_too_large",
                            "Сокращения статьи превышают допустимый размер HTML",
                        )
                        .with_message(
                            "messages.article_too_large_2",
                            crate::model::message_args([]),
                        ));
                    }
                    self.values.insert(key, value);
                }
            }
        }
        Ok(())
    }
    fn diagnostic(&mut self, diagnostic: Diagnostic) -> Result<()> {
        self.ctx.emit(CoreEvent::Diagnostic(diagnostic));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn повторное_сокращение_использует_снимок_текущего_чтения()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("abbr.dsl");
        std::fs::write(&path, "n.\n существительное\n")?;
        let source = PreparedFile {
            id: "abbr".into(),
            display_name: "abbr.dsl".into(),
            path: path.clone(),
        };
        let bundle = InputBundle {
            includes: vec![],
            main: source.clone(),
            abbreviations: Some(source),
            resources: vec![],
        };
        let ctx = RunContext::new(temp.path().to_owned());
        let mut cache = AbbreviationCache::default();
        let wanted = HashSet::from(["n.".to_owned()]);
        assert_eq!(
            cache
                .resolve(&bundle, &ReadOptions::default(), &wanted, &ctx)?
                .get("n.")
                .map(String::as_str),
            Some("существительное")
        );
        std::fs::remove_file(path)?;
        assert_eq!(
            cache
                .resolve(&bundle, &ReadOptions::default(), &wanted, &ctx)?
                .get("n.")
                .map(String::as_str),
            Some("существительное")
        );
        Ok(())
    }
}
