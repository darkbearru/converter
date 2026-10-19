use super::references::inline_text;
use crate::model::{ConverterError, Diagnostic, DiagnosticSeverity, Result, RunContext};

pub(super) fn token_end(text: &str) -> Option<usize> {
    // Апостроф здесь является именем тега ударения, а не кавычкой аргумента.
    if text.starts_with("[']") {
        return Some(2);
    }
    if text.starts_with("[/']") {
        return Some(3);
    }
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

pub(super) fn reference_end(text: &str) -> Option<usize> {
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
            | "!trs"
            | "trs"
            | "!trn"
            | "ex"
            | "com"
            | "sup"
            | "sub"
            | "t"
            | "lang"
            | "ref"
            | "url"
            | "s"
            | "'"
            | "*"
    )
}

fn formatting(name: &str) -> bool {
    // Служебные обёртки не меняют отображение: их можно переносить вместе
    // с оформлением, не разрывая ссылки, ресурсы и абзацы.
    matches!(
        name,
        "b" | "i"
            | "u"
            | "c"
            | "trn"
            | "!trs"
            | "trs"
            | "!trn"
            | "ex"
            | "com"
            | "sup"
            | "sub"
            | "t"
            | "lang"
            | "'"
            | "*"
    )
}

#[derive(Default)]
struct Frame {
    name: String,
    argument: String,
    children: Vec<Node>,
    source_position: usize,
}

impl Frame {
    fn reopened(&self) -> Self {
        Self {
            name: self.name.clone(),
            argument: self.argument.clone(),
            children: Vec::new(),
            source_position: self.source_position,
        }
    }
}

struct MarkupParser<'a> {
    source: &'a str,
    position: usize,
    display: &'a str,
    diagnostics: Vec<Diagnostic>,
    minimum_html_bytes: usize,
    ctx: &'a RunContext,
}

impl MarkupParser<'_> {
    fn push(&mut self, frames: &mut [Frame], node: Node) -> Result<()> {
        // Нижняя оценка размера HTML ограничивает рост дерева при разбиении тегов.
        self.minimum_html_bytes = self.minimum_html_bytes.saturating_add(match &node {
            Node::Text(text) => text.len(),
            Node::Reference(target) => target.len().saturating_add(8),
            Node::Element { .. } => 8,
        });
        if self.minimum_html_bytes > self.ctx.limits.max_html_bytes {
            return Err(ConverterError::new(
                "article_too_large",
                "HTML статьи превышает допустимый размер",
            )
            .with_message("messages.article_too_large", crate::model::message_args([])));
        }
        frames
            .last_mut()
            .ok_or_else(|| {
                ConverterError::new("invalid_markup", "Неожиданный конец разметки")
                    .with_message("messages.invalid_markup", crate::model::message_args([]))
            })?
            .children
            .push(node);
        Ok(())
    }

    fn close(&mut self, frames: &mut Vec<Frame>) -> Result<()> {
        let frame = frames.pop().ok_or_else(|| {
            ConverterError::new("invalid_markup", "Неожиданный конец разметки")
                .with_message("messages.invalid_markup", crate::model::message_args([]))
        })?;
        if frame.children.is_empty() && formatting(&frame.name) {
            return Ok(());
        }
        self.push(
            frames,
            Node::Element {
                name: frame.name,
                argument: frame.argument,
                children: frame.children,
            },
        )
    }

    fn nodes(&mut self) -> Result<Vec<Node>> {
        let mut frames = vec![Frame::default()];
        let mut text = String::new();
        let mut iterations = 0usize;
        let source = self.source;
        while self.position < self.source.len() {
            if iterations.is_multiple_of(1024) {
                self.ctx.control.check()?;
            }
            iterations += 1;
            let rest = &source[self.position..];
            let ch = rest.chars().next().ok_or_else(|| {
                ConverterError::new("invalid_markup", "Неожиданный конец разметки")
                    .with_message("messages.invalid_markup", crate::model::message_args([]))
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
                        self.push(&mut frames, Node::Text(std::mem::take(&mut text)))?;
                    }
                    self.push(
                        &mut frames,
                        Node::Reference(inline_text(target[..end].trim(), self.display)),
                    )?;
                    self.position += end + 4;
                } else {
                    return Err(
                        ConverterError::new("invalid_markup", "Не закрыта ссылка <<…>>")
                            .with_message(
                                "messages.invalid_markup_2",
                                crate::model::message_args([]),
                            ),
                    );
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
                            message_key: Some("messages.unknown_tag".into()),
                            message_args: serde_json::from_value(crate::model::message_args([(
                                "raw",
                                raw.to_string(),
                            )]))
                            .unwrap_or_default(),
                            code: "unknown_tag".into(),
                            message: format!("Неизвестный тег сохранён как текст: [{raw}]"),
                            source_location: None,
                            severity: DiagnosticSeverity::Warning,
                            occurrences: 1,
                        });
                    }
                    continue;
                }
                if !text.is_empty() {
                    self.push(&mut frames, Node::Text(std::mem::take(&mut text)))?;
                }
                if closing {
                    let matching = frames
                        .iter()
                        .rposition(|frame| normalized(&frame.name) == normalized(&name));
                    let Some(matching) = matching else {
                        // [/m] сбрасывает отступ; повторный сброс допустим после смены [mN].
                        if normalized(&name) == "m" {
                            continue;
                        }
                        return Err(ConverterError::new(
                            "invalid_markup",
                            format!("Неожиданное закрытие тега: [{raw}]"),
                        )
                        .with_message(
                            "messages.invalid_markup_3",
                            crate::model::message_args([("raw", raw.to_string())]),
                        ));
                    };
                    let crossed = matching + 1 != frames.len();
                    // После переноса оформления внутрь [p] порядок кадров может отличаться
                    // от исходного: предупреждаем только о пересечении исходных тегов.
                    let source_crossed = frames
                        .iter()
                        .any(|frame| frame.source_position > frames[matching].source_position);
                    if crossed
                        && ((!formatting(&frames[matching].name)
                            && frames[matching].name != "p"
                            && normalized(&frames[matching].name) != "m")
                            || frames[matching + 1..]
                                .iter()
                                .any(|frame| !formatting(&frame.name)))
                    {
                        return Err(ConverterError::new(
                            "invalid_markup",
                            format!("Неожиданное закрытие тега: [{raw}]"),
                        )
                        .with_message(
                            "messages.invalid_markup_3",
                            crate::model::message_args([("raw", raw.to_string())]),
                        ));
                    }
                    let reopen: Vec<_> =
                        frames[matching + 1..].iter().map(Frame::reopened).collect();
                    while frames.len() > matching {
                        self.close(&mut frames)?;
                    }
                    // Восстанавливаем только оформление: ссылки, ресурсы и блоки не разрываем.
                    frames.extend(reopen);
                    if source_crossed {
                        self.diagnostics.push(Diagnostic {
                            message_key: Some("messages.repaired_markup".into()),
                            message_args: serde_json::from_value(crate::model::message_args([(
                                "raw",
                                raw.to_string(),
                            )]))
                            .unwrap_or_default(),
                            code: "repaired_markup".into(),
                            message: format!(
                                "Восстановлено пересечение тегов оформления перед [{raw}]"
                            ),
                            source_location: None,
                            severity: DiagnosticSeverity::Info,
                            occurrences: 1,
                        });
                    }
                    continue;
                }
                // [mN] задаёт текущий абзац: следующий [mN] завершает предыдущий.
                if normalized(&name) == "m"
                    && let Some(previous) = frames
                        .iter()
                        .rposition(|frame| normalized(&frame.name) == "m")
                    && frames[previous + 1..]
                        .iter()
                        .all(|frame| formatting(&frame.name))
                {
                    let styles: Vec<_> =
                        frames[previous + 1..].iter().map(Frame::reopened).collect();
                    while frames.len() > previous {
                        self.close(&mut frames)?;
                    }
                    frames.extend(styles);
                }
                if frames.len() > 128 {
                    return Err(ConverterError::new(
                        "invalid_markup",
                        "Слишком глубокая вложенность тегов DSL",
                    )
                    .with_message("messages.invalid_markup_4", crate::model::message_args([])));
                }
                // Сокращение и абзац остаются цельными узлами, даже если внутри них
                // закрывается внешнее оформление. Переносим только активные стили.
                let reopen = if name == "p" || normalized(&name) == "m" {
                    let start = frames
                        .iter()
                        .rposition(|frame| !formatting(&frame.name))
                        .map_or(0, |position| position + 1);
                    let reopen = frames[start..]
                        .iter()
                        .map(Frame::reopened)
                        .collect::<Vec<_>>();
                    while frames.len() > start {
                        self.close(&mut frames)?;
                    }
                    reopen
                } else {
                    Vec::new()
                };
                frames.push(Frame {
                    name,
                    argument: argument.into(),
                    children: Vec::new(),
                    source_position: self.position,
                });
                frames.extend(reopen);
            } else {
                text.push(ch);
                self.position += ch.len_utf8();
            }
        }
        // Закрытие [/m] необязательно в конце статьи; остальные теги проверяем строго.
        if frames.len() > 1
            && frames[1..]
                .iter()
                .all(|frame| normalized(&frame.name) == "m")
        {
            if !text.is_empty() {
                self.push(&mut frames, Node::Text(std::mem::take(&mut text)))?;
            }
            while frames.len() > 1 {
                self.close(&mut frames)?;
            }
        }
        if frames.len() > 1 {
            let stop = normalized(&frames[frames.len() - 1].name);
            return Err(
                ConverterError::new("invalid_markup", format!("Не закрыт тег [{stop}]"))
                    .with_message(
                        "messages.invalid_markup_5",
                        crate::model::message_args([("stop", stop.to_string())]),
                    ),
            );
        }
        if !text.is_empty() {
            self.push(&mut frames, Node::Text(text))?;
        }
        Ok(frames.remove(0).children)
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
        minimum_html_bytes: 0,
        ctx,
    };
    let nodes = parser.nodes()?;
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
