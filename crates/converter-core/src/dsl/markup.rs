use super::references::inline_text;
use crate::model::{ConverterError, Diagnostic, Result, RunContext};

fn token_end(text: &str) -> Option<usize> {
    let mut quote = None;
    let mut escaped = false;
    for (position, ch) in text.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(open) = quote {
            if ch == open {
                quote = None;
            }
        } else if matches!(ch, '\'' | '"') {
            quote = Some(ch);
        } else if ch == ']' {
            return Some(position);
        }
    }
    None
}

fn reference_end(text: &str) -> Option<usize> {
    let mut escaped = false;
    for (position, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if text[position..].starts_with(">>") {
            return Some(position);
        }
    }
    None
}

#[derive(Debug)]
pub(super) enum Node {
    Text(String),
    Element {
        name: String,
        argument: String,
        children: Vec<Node>,
    },
    Reference(String),
}

fn normalized(name: &str) -> String {
    if name == "m"
        || (name.starts_with('m') && name.len() == 2 && name.as_bytes()[1].is_ascii_digit())
    {
        "m".into()
    } else {
        name.into()
    }
}

fn known(name: &str) -> bool {
    matches!(
        normalized(name).as_str(),
        "b" | "i"
            | "u"
            | "c"
            | "m"
            | "p"
            | "trn"
            | "ex"
            | "com"
            | "sup"
            | "sub"
            | "t"
            | "lang"
            | "ref"
            | "url"
            | "s"
    )
}

struct MarkupParser<'a> {
    source: &'a str,
    position: usize,
    display: &'a str,
    diagnostics: Vec<Diagnostic>,
    ctx: &'a RunContext,
}

impl MarkupParser<'_> {
    fn nodes(&mut self, stop: Option<&str>, depth: usize) -> Result<Vec<Node>> {
        if depth > 128 {
            return Err(ConverterError::new(
                "invalid_markup",
                "Слишком глубокая вложенность тегов DSL",
            ));
        }
        let mut nodes = Vec::new();
        let mut text = String::new();
        let mut iterations = 0usize;
        while self.position < self.source.len() {
            if iterations.is_multiple_of(1024) {
                self.ctx.control.check()?;
            }
            iterations += 1;
            let rest = &self.source[self.position..];
            let ch = rest.chars().next().ok_or_else(|| {
                ConverterError::new("invalid_markup", "Неожиданный конец разметки")
            })?;
            if ch == '\\' {
                self.position += 1;
                if let Some(escaped) = self.source[self.position..].chars().next() {
                    text.push(escaped);
                    self.position += escaped.len_utf8();
                } else {
                    text.push('\\');
                }
            } else if ch == '~' {
                text.push_str(self.display);
                self.position += 1;
            } else if let Some(target) = rest.strip_prefix("<<") {
                if let Some(end) = reference_end(target) {
                    if !text.is_empty() {
                        nodes.push(Node::Text(std::mem::take(&mut text)));
                    }
                    nodes.push(Node::Reference(inline_text(
                        target[..end].trim(),
                        self.display,
                    )));
                    self.position += end + 4;
                } else {
                    return Err(ConverterError::new(
                        "invalid_markup",
                        "Не закрыта ссылка <<…>>",
                    ));
                }
            } else if ch == '[' {
                let Some(end) = token_end(rest) else {
                    text.push(ch);
                    self.position += 1;
                    continue;
                };
                let raw = &rest[1..end];
                let closing = raw.starts_with('/');
                let payload = raw.strip_prefix('/').unwrap_or(raw).trim();
                let (name, argument) = payload
                    .split_once(char::is_whitespace)
                    .unwrap_or((payload, ""));
                let name = name.to_ascii_lowercase();
                self.position += end + 1;
                if !known(&name) {
                    text.push_str(&rest[..end + 1]);
                    if !closing {
                        self.diagnostics.push(Diagnostic {
                            code: "unknown_tag".into(),
                            message: format!("Неизвестный тег сохранён как текст: [{raw}]"),
                            source_location: None,
                        });
                    }
                    continue;
                }
                if !text.is_empty() {
                    nodes.push(Node::Text(std::mem::take(&mut text)));
                }
                if closing {
                    if stop == Some(normalized(&name).as_str()) {
                        return Ok(nodes);
                    }
                    return Err(ConverterError::new(
                        "invalid_markup",
                        format!("Неожиданное закрытие тега: [{raw}]"),
                    ));
                }
                let children = self.nodes(Some(&normalized(&name)), depth + 1)?;
                nodes.push(Node::Element {
                    name,
                    argument: argument.into(),
                    children,
                });
            } else {
                text.push(ch);
                self.position += ch.len_utf8();
            }
        }
        if let Some(stop) = stop {
            return Err(ConverterError::new(
                "invalid_markup",
                format!("Не закрыт тег [{stop}]"),
            ));
        }
        if !text.is_empty() {
            nodes.push(Node::Text(text));
        }
        Ok(nodes)
    }
}

pub(super) fn parse(
    source: &str,
    display: &str,
    ctx: &RunContext,
) -> Result<(Vec<Node>, Vec<Diagnostic>)> {
    let mut parser = MarkupParser {
        source,
        position: 0,
        display,
        diagnostics: Vec::new(),
        ctx,
    };
    let nodes = parser.nodes(None, 0)?;
    Ok((nodes, parser.diagnostics))
}

pub(super) fn plain(nodes: &[Node]) -> String {
    let mut output = String::new();
    for node in nodes {
        match node {
            Node::Text(text) | Node::Reference(text) => output.push_str(text),
            Node::Element { children, .. } => output.push_str(&plain(children)),
        }
    }
    output
}
