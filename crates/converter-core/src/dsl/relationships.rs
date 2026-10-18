use super::{
    decoder,
    markup::{reference_end, token_end},
    metadata, parser,
    references::{argument, inline_text},
};
use crate::{formats::SourceRelationships, model::*};
use std::collections::BTreeSet;

struct References<'a> {
    names: BTreeSet<String>,
    ctx: &'a RunContext,
}

impl parser::AstSink for References<'_> {
    fn push(&mut self, article: parser::ArticleAst) -> Result<()> {
        let mut position = 0;
        while position < article.body.len() {
            self.ctx.control.check()?;
            let rest = &article.body[position..];
            if rest.starts_with('\\') {
                position += 1;
                if let Some(ch) = article.body[position..].chars().next() {
                    position += ch.len_utf8();
                }
            } else if let Some(target) = rest.strip_prefix("<<") {
                let end = reference_end(target).ok_or_else(|| {
                    ConverterError::new("invalid_markup", "Не закрыта ссылка <<…>>")
                })?;
                position += end + 4;
            } else if rest.starts_with('[') {
                if let Some(end) = token_end(rest) {
                    let token = rest[1..end].trim();
                    let (name, args) = token.split_once(char::is_whitespace).unwrap_or((token, ""));
                    if name.eq_ignore_ascii_case("ref")
                        && let Some(target) = argument(args, "dict")
                    {
                        let target = inline_text(target, "~");
                        if !target.trim().is_empty() {
                            self.names.insert(target);
                        }
                        if self.names.len() > self.ctx.limits.max_archive_entries {
                            return Err(ConverterError::new(
                                "analysis_limit",
                                "Слишком много межсловарных зависимостей",
                            ));
                        }
                    }
                    position += end + 1;
                } else {
                    position += 1;
                }
            } else {
                position += rest.find(['[', '\\', '<']).unwrap_or(rest.len()).max(1);
            }
        }
        Ok(())
    }
    fn diagnostic(&mut self, _: Diagnostic) -> Result<()> {
        Ok(())
    }
}

pub(super) fn inspect(
    bundle: &InputBundle,
    options: &ReadOptions,
    ctx: &RunContext,
) -> Result<SourceRelationships> {
    let encoding = decoder::resolve_encoding(&bundle.main, options.encoding.clone(), ctx)?;
    if encoding.needs_confirmation {
        return Err(ConverterError::new(
            "encoding_required",
            "Подтвердите кодировку перед анализом пакета",
        ));
    }
    let meta = metadata::inspect_metadata(bundle, &encoding, ctx)?;
    let mut references = References {
        names: BTreeSet::new(),
        ctx,
    };
    parser::parse_raw(bundle, &encoding, &mut references, ctx)?;
    Ok(SourceRelationships {
        title: meta.title,
        external_dictionaries: references.names.into_iter().collect(),
    })
}
