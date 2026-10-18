use super::{
    decoder::{ResolvedEncoding, SourceLine, decode_lines, open_source},
    lexer::Lexer,
    metadata::directive,
};
use crate::{formats::ReadSummary, model::*};

#[derive(Debug)]
pub struct ArticleAst {
    pub id: u64,
    pub source_line: u64,
    pub parent_id: Option<u64>,
    pub headwords: Vec<String>,
    pub body: String,
}

pub(super) trait AstSink {
    fn push(&mut self, article: ArticleAst) -> Result<()>;
    fn diagnostic(&mut self, diagnostic: Diagnostic) -> Result<()>;
}

struct Group {
    root: ArticleAst,
    children: Vec<ArticleAst>,
    child: Option<ArticleAst>,
    bytes: usize,
}

struct Parser<'a> {
    source: &'a str,
    ctx: &'a RunContext,
    sink: &'a mut dyn AstSink,
    lexer: Lexer,
    group: Option<Group>,
    heads: Vec<String>,
    head_line: u64,
    next_id: u64,
    summary: ReadSummary,
}

pub(super) fn parse_raw(
    bundle: &InputBundle,
    encoding: &ResolvedEncoding,
    sink: &mut dyn AstSink,
    ctx: &RunContext,
) -> Result<ReadSummary> {
    let mut parser = Parser {
        source: &bundle.main.display_name,
        ctx,
        sink,
        lexer: Lexer::default(),
        group: None,
        heads: Vec::new(),
        head_line: 0,
        next_id: 0,
        summary: ReadSummary::default(),
    };
    decode_lines(open_source(&bundle.main)?, encoding.encoding, ctx, |line| {
        parser.line(line)?;
        Ok(true)
    })?;
    if parser.lexer.in_comment {
        return Err(parser.error("Не закрыт комментарий", parser.head_line));
    }
    parser.flush_group()?;
    if !parser.heads.is_empty() {
        return Err(parser.error("У заголовка нет тела статьи", parser.head_line));
    }
    if parser.summary.source_articles == 0 {
        return Err(ConverterError::new(
            "empty_dictionary",
            "Словарь не содержит статей",
        ));
    }
    Ok(parser.summary)
}

impl Parser<'_> {
    fn error(&self, message: &str, line: u64) -> ConverterError {
        ConverterError::new("invalid_dsl", message).at(self.source, line)
    }

    fn line(&mut self, line: SourceLine) -> Result<()> {
        self.ctx.control.check()?;
        let text = self.lexer.line(&line.text);
        if text.trim().is_empty() {
            if let Some(group) = &mut self.group {
                Self::body(group, "", line.number, self.ctx, self.source)?;
            }
            return Ok(());
        }
        if text.starts_with('#') {
            if self.group.is_some() || !self.heads.is_empty() {
                return Err(self.error("Директива находится после начала статьи", line.number));
            }
            if !directive(&text, &mut DictionaryMetadata::default())
                .map_err(|e| e.at(self.source, line.number))?
            {
                self.summary.warnings += 1;
                self.sink.diagnostic(Diagnostic {
                    code: "unknown_directive".into(),
                    message: format!("Неизвестная директива: {text}"),
                    source_location: Some(SourceLocation {
                        source: self.source.into(),
                        line: line.number,
                    }),
                })?;
            }
            return Ok(());
        }
        let indented = text.starts_with([' ', '\t']);
        if !indented {
            self.flush_group()?;
            if self.heads.len() >= self.ctx.limits.max_headword_variants {
                return Err(ConverterError::new(
                    "headword_variants_limit",
                    "Превышен лимит заголовков статьи",
                )
                .at(self.source, line.number));
            }
            if self.heads.is_empty() {
                self.head_line = line.number;
            }
            let size = self
                .heads
                .iter()
                .map(String::len)
                .sum::<usize>()
                .saturating_add(text.len());
            if size > self.ctx.limits.max_article_bytes {
                return Err(ConverterError::new(
                    "article_too_large",
                    "Заголовки статьи превышают допустимый размер",
                )
                .at(self.source, line.number));
            }
            self.heads.push(text.trim_end().into());
            return Ok(());
        }
        if self.group.is_none() {
            if self.heads.is_empty() {
                return Err(self.error("Тело статьи не имеет заголовка", line.number));
            }
            let root = ArticleAst {
                id: self.next_id,
                source_line: self.head_line,
                parent_id: None,
                headwords: std::mem::take(&mut self.heads),
                body: String::new(),
            };
            self.next_id += 1;
            self.group = Some(Group {
                bytes: root.headwords.iter().map(String::len).sum(),
                root,
                children: Vec::new(),
                child: None,
            });
        }
        let content = &text[1..];
        if let Some(marker) = content.trim_start().strip_prefix('@') {
            let name = marker.trim();
            let group = self
                .group
                .as_mut()
                .ok_or_else(|| ConverterError::new("invalid_dsl", "Нет основной статьи"))?;
            if name.is_empty() {
                let child = group.child.take().ok_or_else(|| {
                    ConverterError::new("invalid_dsl", "Закрытие подстатьи без открытия")
                        .at(self.source, line.number)
                })?;
                if child.body.trim().is_empty() {
                    return Err(
                        ConverterError::new("invalid_dsl", "Подстатья не имеет тела")
                            .at(self.source, line.number),
                    );
                }
                group.children.push(child);
            } else if let Some(child) = group.child.as_mut().filter(|child| child.body.is_empty()) {
                if child.headwords.len() >= self.ctx.limits.max_headword_variants {
                    return Err(ConverterError::new(
                        "headword_variants_limit",
                        "Превышен лимит заголовков подстатьи",
                    )
                    .at(self.source, line.number));
                }
                child.headwords.push(name.into());
            } else {
                if let Some(child) = group.child.take() {
                    group.children.push(child);
                }
                group.root.body.push_str(
                    &super::references::subentry_reference(
                        name,
                        self.ctx.limits.max_headword_variants,
                    )
                    .map_err(|error| error.at(self.source, line.number))?,
                );
                group.child = Some(ArticleAst {
                    id: self.next_id,
                    source_line: line.number,
                    parent_id: Some(group.root.id),
                    headwords: vec![name.into()],
                    body: String::new(),
                });
                self.next_id += 1;
            }
            group.bytes = group.bytes.saturating_add(content.len());
            Self::check_size(group, self.ctx, self.source, line.number)?;
        } else {
            Self::body(
                self.group
                    .as_mut()
                    .ok_or_else(|| ConverterError::new("invalid_dsl", "Нет статьи"))?,
                content,
                line.number,
                self.ctx,
                self.source,
            )?;
        }
        Ok(())
    }

    fn check_size(group: &Group, ctx: &RunContext, source: &str, line: u64) -> Result<()> {
        if group.bytes > ctx.limits.max_article_bytes {
            return Err(ConverterError::new(
                "article_too_large",
                "Тело статьи вместе с подстатьями превышает допустимый размер",
            )
            .at(source, line));
        }
        Ok(())
    }

    fn body(
        group: &mut Group,
        content: &str,
        line: u64,
        ctx: &RunContext,
        source: &str,
    ) -> Result<()> {
        group.bytes = group.bytes.saturating_add(content.len()).saturating_add(1);
        Self::check_size(group, ctx, source, line)?;
        let target = group.child.as_mut().unwrap_or(&mut group.root);
        target.body.push_str(content);
        target.body.push('\n');
        Ok(())
    }

    fn flush_group(&mut self) -> Result<()> {
        let Some(group) = self.group.take() else {
            return Ok(());
        };
        if let Some(child) = group.child {
            return Err(self.error("Подстатья не закрыта маркером @", child.source_line));
        }
        if group.root.body.trim().is_empty() {
            return Err(self.error("У статьи нет тела", group.root.source_line));
        }
        self.emit(group.root)?;
        for child in group.children {
            self.emit(child)?;
        }
        Ok(())
    }

    fn emit(&mut self, ast: ArticleAst) -> Result<()> {
        self.ctx.control.check()?;
        let line = ast.source_line;
        self.sink.push(ast).map_err(|error| {
            if error.source_location.is_none() {
                error.at(self.source, line)
            } else {
                error
            }
        })?;
        self.summary.source_articles += 1;
        Ok(())
    }
}
