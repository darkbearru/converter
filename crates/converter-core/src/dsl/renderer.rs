use super::{
    abbreviations,
    headwords::display_headword,
    markup::{self, Node},
    parser::ArticleAst,
    references::{argument, escape_html, inline_text, uri},
};
use crate::{model::*, prepare::resolve_resource};
use std::collections::{HashMap, HashSet};

pub const CONVERTER_CSS: &str = include_str!("../../assets/converter.css");

#[derive(Debug)]
pub struct RenderedArticle {
    pub html: String,
    pub resource_keys: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn render_article(
    ast: &ArticleAst,
    bundle: &InputBundle,
    options: &ReadOptions,
) -> Result<RenderedArticle> {
    render_with_context(
        ast,
        bundle,
        options,
        &RunContext::new(std::env::temp_dir()),
        &mut abbreviations::AbbreviationCache::default(),
    )
}

pub(super) fn render_with_context(
    ast: &ArticleAst,
    bundle: &InputBundle,
    options: &ReadOptions,
    ctx: &RunContext,
    cache: &mut abbreviations::AbbreviationCache,
) -> Result<RenderedArticle> {
    ctx.control.check()?;
    let display = display_headword(ast.headwords.first().map_or("", String::as_str))?;
    let (nodes, diagnostics) = markup::parse(&ast.body, &display, ctx)?;
    let mut wanted = HashSet::new();
    fn collect(nodes: &[Node], wanted: &mut HashSet<String>) {
        for node in nodes {
            if let Node::Element { name, children, .. } = node {
                if name == "p" {
                    wanted.insert(markup::plain(children).trim().into());
                }
                collect(children, wanted);
            }
        }
    }
    collect(&nodes, &mut wanted);
    // В памяти остаются только сокращения, использованные одной статьёй.
    let abbreviations = cache.resolve(bundle, options, &wanted, ctx)?;
    let mut renderer = Renderer {
        bundle,
        ctx,
        display: &display,
        abbreviations,
        result: RenderedArticle {
            html: String::new(),
            resource_keys: Vec::new(),
            diagnostics,
        },
    };
    renderer.write("<link rel=\"stylesheet\" href=\"converter.css\"><article class=\"dsl-article\"><header class=\"dsl-headword\">")?;
    renderer.write(&escape_html(&display))?;
    renderer.write("</header><div class=\"dsl-body\">")?;
    renderer.nodes(&nodes)?;
    renderer.write("</div></article>")?;
    for diagnostic in &mut renderer.result.diagnostics {
        diagnostic.source_location = Some(SourceLocation {
            source: bundle.main.display_name.clone(),
            line: ast.source_line,
        });
    }
    Ok(renderer.result)
}

struct Renderer<'a> {
    bundle: &'a InputBundle,
    ctx: &'a RunContext,
    display: &'a str,
    abbreviations: HashMap<String, String>,
    result: RenderedArticle,
}

impl Renderer<'_> {
    fn write(&mut self, value: &str) -> Result<()> {
        if self.result.html.len().saturating_add(value.len()) > self.ctx.limits.max_html_bytes {
            return Err(ConverterError::new(
                "article_too_large",
                "HTML статьи превышает допустимый размер",
            ));
        }
        self.result.html.push_str(value);
        Ok(())
    }
    fn warning(&mut self, code: &str, message: String) {
        self.result.diagnostics.push(Diagnostic {
            code: code.into(),
            message,
            source_location: None,
        });
    }
    fn nodes(&mut self, nodes: &[Node]) -> Result<()> {
        for node in nodes {
            self.ctx.control.check()?;
            match node {
                Node::Text(text) => self.write(&escape_html(text))?,
                Node::Reference(target) => {
                    self.write(&format!(
                        "<a href=\"entry://{}\">{}</a>",
                        uri(target, false),
                        escape_html(target)
                    ))?;
                }
                Node::Element {
                    name,
                    argument: arg,
                    children,
                } => match name.as_str() {
                    "ref" => {
                        let plain = markup::plain(children);
                        let target = argument(arg, "target").map_or_else(
                            || plain.trim().to_owned(),
                            |target| inline_text(target, self.display),
                        );
                        self.write(&format!("<a href=\"entry://{}\">", uri(&target, false)))?;
                        self.nodes(children)?;
                        self.write("</a>")?;
                    }
                    "url" => {
                        let plain = markup::plain(children);
                        let target = argument(arg, "target").unwrap_or(if arg.is_empty() {
                            plain.trim()
                        } else {
                            arg.trim()
                        });
                        let lower = target.to_ascii_lowercase();
                        let valid = (lower.starts_with("https://") || lower.starts_with("http://"))
                            && !target.chars().any(char::is_control);
                        if valid {
                            self.write(&format!(
                                "<a href=\"{}\" rel=\"noreferrer noopener\">",
                                escape_html(target)
                            ))?;
                            self.nodes(children)?;
                            self.write("</a>")?;
                        } else {
                            self.warning(
                                "unsafe_url",
                                "Ссылка с неподдерживаемой схемой сохранена как текст".into(),
                            );
                            self.nodes(children)?;
                        }
                    }
                    "s" => self.resource(&markup::plain(children))?,
                    "p" => {
                        let text = markup::plain(children);
                        if let Some(title) = self.abbreviations.get(text.trim()) {
                            self.write(&format!(
                                "<abbr class=\"dsl-abbreviation\" title=\"{}\">",
                                escape_html(title)
                            ))?;
                        } else {
                            self.warning(
                                "missing_abbreviation",
                                format!("Не найдено сокращение: {}", text.trim()),
                            );
                            self.write("<abbr class=\"dsl-abbreviation\">")?;
                        }
                        self.nodes(children)?;
                        self.write("</abbr>")?;
                    }
                    "c" => {
                        let color = arg.trim();
                        let valid = (!color.is_empty()
                            && color.len() <= 32
                            && color.bytes().all(|b| b.is_ascii_alphabetic()))
                            || (color.starts_with('#')
                                && [4, 5, 7, 9].contains(&color.len())
                                && color[1..].bytes().all(|b| b.is_ascii_hexdigit()));
                        if color.is_empty() {
                            self.write("<span class=\"dsl-color\">")?;
                        } else if valid {
                            self.write(&format!(
                                "<span class=\"dsl-color\" style=\"--dsl-color:{color}\">"
                            ))?;
                        } else {
                            self.warning(
                                "invalid_color",
                                "Недопустимое значение цвета заменено стандартным".into(),
                            );
                            self.write("<span class=\"dsl-color\">")?;
                        }
                        self.nodes(children)?;
                        self.write("</span>")?;
                    }
                    "lang" => {
                        let id = argument(arg, "id").unwrap_or_default();
                        let language = match id {
                            "1033" | "2057" => "en",
                            "1049" => "ru",
                            "1031" => "de",
                            "1036" => "fr",
                            "2052" | "1028" => "zh",
                            "1041" => "ja",
                            _ => "und",
                        };
                        self.write(&format!("<span class=\"dsl-language\" lang=\"{language}\" data-dsl-language=\"{}\">", escape_html(arg)))?;
                        self.nodes(children)?;
                        self.write("</span>")?;
                    }
                    name if name == "m" || name.starts_with('m') => {
                        let level = name
                            .strip_prefix('m')
                            .filter(|v| !v.is_empty())
                            .unwrap_or("0");
                        self.write(&format!("<div class=\"dsl-indent dsl-indent-{level}\">"))?;
                        self.nodes(children)?;
                        self.write("</div>")?;
                    }
                    name => {
                        let (open, close) = match name {
                            "b" => ("<strong>", "</strong>"),
                            "i" => ("<em>", "</em>"),
                            "u" => ("<span class=\"dsl-underline\">", "</span>"),
                            "sup" => ("<sup>", "</sup>"),
                            "sub" => ("<sub>", "</sub>"),
                            "trn" => ("<span class=\"dsl-translation\">", "</span>"),
                            "ex" => ("<span class=\"dsl-example\">", "</span>"),
                            "com" => ("<span class=\"dsl-comment\">", "</span>"),
                            "t" => ("<span class=\"dsl-transcription\">", "</span>"),
                            _ => {
                                return Err(ConverterError::new(
                                    "invalid_markup",
                                    "Неизвестный узел разметки",
                                ));
                            }
                        };
                        self.write(open)?;
                        self.nodes(children)?;
                        self.write(close)?;
                    }
                },
            }
        }
        Ok(())
    }
    fn resource(&mut self, raw: &str) -> Result<()> {
        let raw = raw.trim();
        let Some(resource) = resolve_resource(self.bundle, raw)? else {
            self.warning("missing_resource", format!("Отсутствует ресурс: {raw}"));
            return self.write(&format!(
                "<span class=\"dsl-missing-resource\">[Отсутствует ресурс: {}]</span>",
                escape_html(raw)
            ));
        };
        let relative = resource.key.trim_start_matches('\\').replace('\\', "/");
        if raw.replace('\\', "/").trim_start_matches('/') != relative {
            self.warning(
                "resource_name_fallback",
                format!("Ресурс {raw} сопоставлен с {relative}"),
            );
        }
        if !self.result.resource_keys.contains(&resource.key) {
            self.result.resource_keys.push(resource.key);
        }
        let extension = relative
            .rsplit('.')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ["png", "jpg", "jpeg", "gif", "bmp", "webp", "svg"].contains(&extension.as_str()) {
            self.write(&format!(
                "<img class=\"dsl-image\" src=\"{}\" alt=\"{}\">",
                uri(&relative, true),
                escape_html(&relative)
            ))
        } else if ["mp3", "wav", "ogg", "flac", "m4a", "aac", "opus", "spx"]
            .contains(&extension.as_str())
        {
            self.write(&format!(
                "<a class=\"dsl-sound\" href=\"sound://{}\">▶ {}</a>",
                uri(&relative, true),
                escape_html(&relative)
            ))
        } else {
            self.warning(
                "unsupported_resource_type",
                format!("Тип ресурса не поддерживается для показа: {relative}"),
            );
            self.write(&format!(
                "<span class=\"dsl-unsupported-resource\">[Ресурс: {}]</span>",
                escape_html(&relative)
            ))
        }
    }
}
