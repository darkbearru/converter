use super::{decoder, lexer::Lexer};
use crate::model::*;

pub(crate) fn include_directive(text: &str) -> Result<Option<String>> {
    let (name, value) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    if name != "#INCLUDE" {
        return Ok(None);
    }
    let value = value.trim();
    if value.len() < 3 || !value.starts_with('"') || !value.ends_with('"') {
        return Err(ConverterError::new(
            "invalid_include",
            "#INCLUDE должен содержать имя файла в кавычках",
        )
        .with_message("messages.invalid_include", message_args([])));
    }
    Ok(Some(
        value[1..value.len() - 1]
            .replace("\\\\", "\\")
            .replace("\\\"", "\""),
    ))
}

pub(crate) fn inspect(
    file: &PreparedFile,
    ctx: &RunContext,
    choice: &EncodingChoice,
) -> Result<Vec<(String, u64)>> {
    let encoding = match resolve_part_encoding(file, choice, ctx) {
        Ok(encoding) => encoding,
        // Оставляем выбор кодировки штатному этапу Inspect. После подтверждения
        // prepare вызывается повторно с выбранной пользователем кодировкой.
        Err(error) if error.code == "encoding_required" => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut lexer = Lexer::default();
    let mut includes = Vec::new();
    decoder::decode_lines(
        decoder::open_source(file)?,
        encoding.encoding,
        ctx,
        |line| {
            let text = lexer.line(&line.text);
            if text.trim().is_empty() {
                return Ok(true);
            }
            if !text.starts_with('#') {
                return Ok(false);
            }
            if let Some(target) =
                include_directive(&text).map_err(|e| e.at(&file.display_name, line.number))?
            {
                if includes.len() >= ctx.limits.max_archive_entries {
                    return Err(ConverterError::new(
                        "include_limit",
                        "Слишком много включённых частей DSL",
                    )
                    .with_message("messages.include_limit", message_args([]))
                    .at(&file.display_name, line.number));
                }
                includes.push((target, line.number));
            }
            Ok(true)
        },
    )?;
    Ok(includes)
}

// Одинаковая политика при поиске вложенных директив и при чтении статей:
// собственный BOM/автоопределение части, ручной выбор — для неоднозначной кодировки.
pub(crate) fn resolve_part_encoding(
    file: &PreparedFile,
    fallback: &EncodingChoice,
    ctx: &RunContext,
) -> Result<decoder::ResolvedEncoding> {
    decoder::resolve_encoding(file, EncodingChoice::Auto, ctx).or_else(|error| {
        if error.code == "encoding_required" && matches!(fallback, EncodingChoice::Manual(_)) {
            decoder::resolve_encoding(file, fallback.clone(), ctx)
        } else {
            Err(error)
        }
    })
}
