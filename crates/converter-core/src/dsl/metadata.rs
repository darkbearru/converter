use super::{
    decoder::{ResolvedEncoding, decode_lines, open_source},
    lexer::Lexer,
};
use crate::model::*;

pub(super) fn directive(text: &str, meta: &mut DictionaryMetadata) -> Result<bool> {
    let (name, value) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let target = match name {
        "#NAME" => &mut meta.title,
        "#INDEX_LANGUAGE" => &mut meta.index_language,
        "#CONTENTS_LANGUAGE" => &mut meta.contents_language,
        _ => return Ok(false),
    };
    let value = value.trim();
    if !value.starts_with('"') || !value.ends_with('"') || value.len() < 2 {
        return Err(ConverterError::new(
            "invalid_dsl",
            format!("Директива {name} должна содержать значение в кавычках"),
        )
        .with_message(
            "messages.invalid_dsl",
            crate::model::message_args([("name", name.to_string())]),
        ));
    }
    *target = value[1..value.len() - 1]
        .replace("\\\"", "\"")
        .replace("\\\\", "\\");
    Ok(true)
}

pub(super) fn inspect_metadata(
    bundle: &InputBundle,
    encoding: &ResolvedEncoding,
    ctx: &RunContext,
) -> Result<DictionaryMetadata> {
    let mut meta = DictionaryMetadata {
        source_encoding: encoding.encoding_name.clone(),
        ..DictionaryMetadata::default()
    };
    let mut lexer = Lexer::default();
    decode_lines(open_source(&bundle.main)?, encoding.encoding, ctx, |line| {
        let text = lexer.line(&line.text);
        if text.trim().is_empty() {
            return Ok(true);
        }
        if text.starts_with('#') {
            directive(&text, &mut meta)
                .map_err(|error| error.at(&bundle.main.display_name, line.number))?;
            Ok(true)
        } else {
            Ok(false)
        }
    })?;
    if meta.title.is_empty() {
        meta.title.clone_from(&bundle.main.display_name);
    }
    Ok(meta)
}
