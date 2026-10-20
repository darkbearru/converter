use super::lexer::Lexer;
use crate::model::*;
use encoding_rs::{
    Encoding, UTF_8, UTF_16BE, UTF_16LE, WINDOWS_874, WINDOWS_1250, WINDOWS_1251, WINDOWS_1252,
    WINDOWS_1253, WINDOWS_1254, WINDOWS_1255, WINDOWS_1256, WINDOWS_1257, WINDOWS_1258,
};

pub(super) fn unsupported(label: &str) -> ConverterError {
    ConverterError::new(
        "unsupported_encoding",
        format!("Кодировка не поддерживается: {label}"),
    )
    .with_message(
        "messages.unsupported_encoding",
        message_args([("label", label.to_owned())]),
    )
}

pub(super) fn manual(label: &str) -> Result<&'static Encoding> {
    Encoding::for_label(label.as_bytes())
        .filter(|encoding| {
            [
                UTF_8,
                UTF_16LE,
                UTF_16BE,
                WINDOWS_874,
                WINDOWS_1250,
                WINDOWS_1251,
                WINDOWS_1252,
                WINDOWS_1253,
                WINDOWS_1254,
                WINDOWS_1255,
                WINDOWS_1256,
                WINDOWS_1257,
                WINDOWS_1258,
            ]
            .contains(encoding)
        })
        .ok_or_else(|| unsupported(label))
}

pub(super) fn declaration(prefix: &[u8], complete: bool) -> Result<Option<&'static Encoding>> {
    // Синтаксис директив и комментариев ASCII-совместим во всех поддержанных ANSI.
    let text = String::from_utf8_lossy(prefix);
    let mut lexer = Lexer::default();
    for raw in text.split_inclusive('\n') {
        if !complete && !raw.ends_with('\n') {
            break;
        }
        let line = lexer.line(raw.trim_end_matches(['\r', '\n']));
        if line.trim().is_empty() {
            continue;
        }
        if !line.starts_with('#') {
            break;
        }
        let (name, value) = line.split_once(char::is_whitespace).unwrap_or((&line, ""));
        if name != "#SOURCE_CODE_PAGE" {
            continue;
        }
        let value = value.trim();
        if value.len() < 2 || !value.starts_with('"') || !value.ends_with('"') {
            return Err(ConverterError::new(
                "invalid_dsl",
                "#SOURCE_CODE_PAGE должна содержать значение в кавычках",
            )
            .with_message(
                "messages.invalid_dsl",
                message_args([("name", name.to_owned())]),
            ));
        }
        let label = &value[1..value.len() - 1];
        return match label {
            "EasternEuropean" => Ok(Some(WINDOWS_1250)),
            "Cyrillic" => Ok(Some(WINDOWS_1251)),
            "Latin" => Ok(Some(WINDOWS_1252)),
            "Greek" => Ok(Some(WINDOWS_1253)),
            "Turkish" => Ok(Some(WINDOWS_1254)),
            "Hebrew" => Ok(Some(WINDOWS_1255)),
            "Arabic" => Ok(Some(WINDOWS_1256)),
            "Baltic" => Ok(Some(WINDOWS_1257)),
            "Vietnamese" => Ok(Some(WINDOWS_1258)),
            "Thai" => Ok(Some(WINDOWS_874)),
            _ => Err(unsupported(label)),
        };
    }
    Ok(None)
}
