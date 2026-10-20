use crate::model::{ConverterError, Result};
use std::{collections::HashSet, iter::Peekable, str::Chars};

enum Part {
    Text(String),
    Optional(Vec<Part>),
    Unsorted,
}
pub struct HeadwordAst {
    parts: Vec<Part>,
}

impl HeadwordAst {
    pub fn parse(raw: &str) -> Result<Self> {
        Ok(Self {
            parts: parse_parts(&mut raw.chars().peekable(), None, 0)?,
        })
    }
}

fn invalid() -> ConverterError {
    ConverterError::new(
        "invalid_headword",
        "Непарная скобка или незавершённое экранирование в заголовке",
    )
    .with_message(
        "messages.invalid_headword_2",
        crate::model::message_args([]),
    )
}
fn variants_limit() -> ConverterError {
    ConverterError::new(
        "headword_variants_limit",
        "Превышен лимит вариантов заголовка",
    )
    .with_message(
        "messages.headword_variants_limit_4",
        crate::model::message_args([]),
    )
}

fn parse_parts(
    chars: &mut Peekable<Chars<'_>>,
    stop: Option<char>,
    depth: usize,
) -> Result<Vec<Part>> {
    if depth > 128 {
        return Err(ConverterError::new(
            "invalid_headword",
            "Слишком глубокая вложенность заголовка",
        )
        .with_message(
            "messages.invalid_headword_3",
            crate::model::message_args([]),
        ));
    }
    let mut parts = Vec::new();
    let mut text = String::new();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => text.push(chars.next().ok_or_else(invalid)?),
            // В несортируемой части скобки служат видимой пунктуацией,
            // например item{(}s{)}; они не образуют поисковые варианты.
            '(' | ')' if stop == Some('}') => text.push(ch),
            ')' | '}' => {
                if Some(ch) != stop {
                    return Err(invalid());
                }
                if !text.is_empty() {
                    parts.push(Part::Text(text));
                }
                return Ok(parts);
            }
            '(' | '{' => {
                if !text.is_empty() {
                    parts.push(Part::Text(std::mem::take(&mut text)));
                }
                let nested =
                    parse_parts(chars, Some(if ch == '(' { ')' } else { '}' }), depth + 1)?;
                parts.push(if ch == '(' {
                    Part::Optional(nested)
                } else {
                    Part::Unsorted
                });
            }
            _ => text.push(ch),
        }
    }
    if stop.is_some() {
        return Err(invalid());
    }
    if !text.is_empty() {
        parts.push(Part::Text(text));
    }
    Ok(parts)
}

fn expand(parts: &[Part], limit: usize) -> Result<Vec<String>> {
    let mut values = vec![String::new()];
    for part in parts {
        match part {
            Part::Text(text) => {
                for value in &mut values {
                    value.push_str(text);
                }
            }
            Part::Unsorted => {}
            Part::Optional(nested) => {
                let suffixes = expand(nested, limit)?;
                let mut next = values.clone();
                let mut seen: HashSet<String> = next.iter().cloned().collect();
                for value in &values {
                    for suffix in &suffixes {
                        let joined = format!("{value}{suffix}");
                        if seen.insert(joined.clone()) {
                            if next.len() >= limit {
                                return Err(variants_limit());
                            }
                            next.push(joined);
                        }
                    }
                }
                values = next;
            }
        }
    }
    Ok(values)
}

pub fn expand_headwords(raw: &str, limit: usize) -> Result<Vec<String>> {
    if limit == 0 {
        return Err(variants_limit());
    }
    let ast = HeadwordAst::parse(raw)?;
    let mut seen = HashSet::new();
    let values: Vec<_> = expand(&ast.parts, limit)?
        .into_iter()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty() && seen.insert(s.clone()))
        .collect();
    if values.is_empty() {
        return Err(ConverterError::new(
            "invalid_headword",
            "Заголовок не содержит поискового ключа",
        )
        .with_message(
            "messages.invalid_headword_4",
            crate::model::message_args([]),
        ));
    }
    Ok(values)
}

pub(super) fn display_source(raw: &str) -> Result<String> {
    // Проверяем структуру, но сохраняем экранирование для разбора разметки.
    // Поисковые варианты раскрываются отдельно; в карточке круглые скобки видны.
    HeadwordAst::parse(raw)?;
    let mut value = String::new();
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                value.push(ch);
                value.push(chars.next().ok_or_else(invalid)?);
            }
            '{' | '}' => {}
            '^' | '~' => {
                // Эти команды действуют в теле статьи, а не в первом заголовке.
                value.push('\\');
                value.push(ch);
            }
            _ => value.push(ch),
        }
    }
    Ok(value.trim().to_owned())
}

pub fn display_headword(raw: &str) -> Result<String> {
    let source = display_source(raw)?;
    if !source.contains(['[', '\\', '<', '^']) {
        return Ok(source);
    }
    let (nodes, _) = super::markup::parse(
        &source,
        "~",
        &crate::model::RunContext::new(std::env::temp_dir()),
    )?;
    Ok(super::markup::plain(&nodes))
}
