use crate::model::*;
use encoding_rs::{DecoderResult, Encoding, UTF_8, UTF_16BE, UTF_16LE, WINDOWS_1251};
use flate2::read::MultiGzDecoder;
use std::{
    fs::File,
    io::{BufReader, Read},
};

#[derive(Debug, Clone)]
pub struct ResolvedEncoding {
    pub encoding_name: String,
    pub needs_confirmation: bool,
    pub encoding: &'static Encoding,
}

#[derive(Debug)]
pub struct SourceLine {
    pub number: u64,
    pub text: String,
}

pub fn open_source(file: &PreparedFile) -> Result<Box<dyn Read>> {
    let mut magic = [0; 2];
    let count = File::open(&file.path)?.read(&mut magic)?;
    let source = BufReader::new(File::open(&file.path)?);
    if count == 2 && magic == [0x1f, 0x8b] {
        Ok(Box::new(MultiGzDecoder::new(source)))
    } else {
        Ok(Box::new(source))
    }
}

pub fn resolve_encoding(
    main: &PreparedFile,
    choice: EncodingChoice,
    ctx: &RunContext,
) -> Result<ResolvedEncoding> {
    ctx.control.check()?;
    let (encoding, needs_confirmation) = match choice {
        EncodingChoice::Manual(label) => {
            let encoding = Encoding::for_label(label.as_bytes())
                .filter(|e| [UTF_8, UTF_16LE, UTF_16BE, WINDOWS_1251].contains(e))
                .ok_or_else(|| {
                    ConverterError::new(
                        "unsupported_encoding",
                        format!("Кодировка не поддерживается: {label}"),
                    )
                    .with_message(
                        "messages.unsupported_encoding",
                        crate::model::message_args([("label", label.to_string())]),
                    )
                })?;
            (encoding, false)
        }
        EncodingChoice::Auto => {
            let mut prefix = Vec::new();
            open_source(main)?
                .take(64 * 1024)
                .read_to_end(&mut prefix)?;
            if prefix.starts_with(&[0xef, 0xbb, 0xbf]) {
                (UTF_8, false)
            } else if prefix.starts_with(&[0xff, 0xfe]) {
                (UTF_16LE, false)
            } else if prefix.starts_with(&[0xfe, 0xff]) {
                (UTF_16BE, false)
            } else if prefix.contains(&0) {
                return Err(ConverterError::new(
                    "encoding_required",
                    "Источник похож на UTF-16 без BOM. Выберите кодировку вручную",
                )
                .with_message("messages.encoding_required", crate::model::message_args([])));
            } else {
                let validation = decode_chunks(
                    open_source(main)?,
                    UTF_8,
                    ctx,
                    Some(ConversionStage::Inspect),
                    |_| Ok(true),
                );
                match validation {
                    Ok(()) => (UTF_8, false),
                    Err(error) if error.code == "invalid_encoding" => (WINDOWS_1251, true),
                    Err(error) => return Err(error),
                }
            }
        }
    };
    Ok(ResolvedEncoding {
        encoding_name: encoding.name().to_ascii_lowercase(),
        needs_confirmation,
        encoding,
    })
}

fn decode_chunks(
    mut reader: impl Read,
    encoding: &'static Encoding,
    ctx: &RunContext,
    stage: Option<ConversionStage>,
    mut consume: impl FnMut(&str) -> Result<bool>,
) -> Result<()> {
    let mut decoder = encoding.new_decoder_with_bom_removal();
    let mut input = [0; 16 * 1024];
    let mut output = String::with_capacity(64 * 1024);
    let mut completed = 0u64;
    loop {
        ctx.control.check()?;
        let size = reader.read(&mut input)?;
        let mut offset = 0;
        loop {
            ctx.control.check()?;
            output.clear();
            let (status, consumed) = decoder.decode_to_string_without_replacement(
                &input[offset..size],
                &mut output,
                size == 0,
            );
            offset += consumed;
            if matches!(status, DecoderResult::Malformed(_, _)) {
                return Err(ConverterError::new(
                    "invalid_encoding",
                    format!(
                        "Некорректная последовательность {} около байта {}",
                        encoding.name(),
                        completed + offset as u64
                    ),
                )
                .with_message(
                    "messages.invalid_encoding",
                    crate::model::message_args([
                        ("arg0", encoding.name().to_string()),
                        ("arg1", format!("{}", completed + offset as u64)),
                    ]),
                ));
            }
            if !consume(&output)? {
                return Ok(());
            }
            if matches!(status, DecoderResult::InputEmpty) {
                break;
            }
        }
        completed = completed.checked_add(size as u64).ok_or_else(|| {
            ConverterError::new("size_overflow", "Переполнение размера источника")
                .with_message("messages.size_overflow_7", crate::model::message_args([]))
        })?;
        if let Some(stage) = stage {
            let event = StageProgress {
                stage,
                completed,
                total: (size == 0).then_some(completed),
                unit: ProgressUnit::Bytes,
                articles: 0,
                resources: 0,
            };
            ctx.emit(if size == 0 {
                CoreEvent::StageCompleted(event)
            } else {
                CoreEvent::Progress(event)
            });
        }
        if size == 0 {
            return Ok(());
        }
    }
}

pub fn decode_lines(
    reader: impl Read,
    encoding: &'static Encoding,
    ctx: &RunContext,
    mut consume: impl FnMut(SourceLine) -> Result<bool>,
) -> Result<()> {
    let mut pending = String::new();
    let mut number = 1;
    let mut stopped = false;
    decode_chunks(reader, encoding, ctx, None, |text| {
        for part in text.split_inclusive('\n') {
            let ends = part.ends_with('\n');
            let content = part.strip_suffix('\n').unwrap_or(part);
            if pending.len().saturating_add(content.len()) > ctx.limits.max_article_bytes {
                return Err(ConverterError::new(
                    "article_too_large",
                    "Строка DSL превышает допустимый размер",
                )
                .with_message(
                    "messages.article_too_large_3",
                    crate::model::message_args([]),
                ));
            }
            pending.push_str(content);
            if ends {
                if pending.ends_with('\r') {
                    pending.pop();
                }
                if !consume(SourceLine {
                    number,
                    text: std::mem::take(&mut pending),
                })? {
                    stopped = true;
                    return Ok(false);
                }
                number += 1;
            }
        }
        Ok(true)
    })?;
    if !stopped && !pending.is_empty() {
        if pending.ends_with('\r') {
            pending.pop();
        }
        consume(SourceLine {
            number,
            text: pending,
        })?;
    }
    Ok(())
}
