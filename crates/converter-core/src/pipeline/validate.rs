use crate::{
    mdict::collation::{mdd_sort_key, mdx_sort_key},
    model::*,
};
use flate2::read::ZlibDecoder;
use std::{
    fs::File,
    io::{self, BufReader, Cursor, Read, Seek, Write},
};

fn invalid(key: &str, message: &str) -> ConverterError {
    ConverterError::new("invalid_mdict", message).with_message(key, crate::model::message_args([]))
}
fn number(reader: &mut impl Read) -> Result<u64> {
    let mut bytes = [0; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_be_bytes(bytes))
}
fn sum(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b)
        .ok_or_else(|| invalid("messages.mdict_size_overflow", "Переполнение размера MDict"))
}

fn block(
    reader: &mut impl Read,
    length: u64,
    expected: u64,
    target: &mut impl Write,
    ctx: &RunContext,
) -> Result<()> {
    if length < 8 {
        return Err(invalid(
            "messages.mdict_short_block",
            "Слишком короткий блок MDict",
        ));
    }
    let mut prefix = [0; 8];
    reader.read_exact(&mut prefix)?;
    if prefix[..4] != 2u32.to_le_bytes() {
        return Err(invalid(
            "messages.mdict_zlib_block",
            "Ожидался блок zlib MDict v2",
        ));
    }
    let checksum = u32::from_be_bytes(
        prefix[4..]
            .try_into()
            .map_err(|_| invalid("messages.mdict_block_header", "Заголовок блока"))?,
    );
    let mut decoder = ZlibDecoder::new(reader.take(length - 8));
    let mut hash = adler2::Adler32::new();
    let mut raw = 0u64;
    let mut buffer = [0; 64 * 1024];
    loop {
        ctx.control.check()?;
        let count = decoder.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        raw = sum(raw, count as u64)?;
        if raw > expected {
            return Err(invalid(
                "messages.mdict_block_size",
                "Блок превышает объявленный размер",
            ));
        }
        hash.write_slice(&buffer[..count]);
        target.write_all(&buffer[..count])?;
    }
    if raw != expected
        || hash.checksum() != checksum
        || decoder.total_in() != length - 8
        || decoder.into_inner().limit() != 0
    {
        return Err(invalid(
            "messages.mdict_block_checksum",
            "Не совпала длина или контрольная сумма блока MDict",
        ));
    }
    Ok(())
}

fn key_string(bytes: Vec<u8>, utf16: bool) -> Result<String> {
    if utf16 {
        if !bytes.len().is_multiple_of(2) {
            return Err(invalid(
                "messages.mdict_odd_utf16",
                "Нечётная длина ключа UTF-16",
            ));
        }
        let units: Vec<_> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        String::from_utf16(&units)
            .map_err(|_| invalid("messages.mdict_utf16_key", "Неправильный ключ UTF-16"))
    } else {
        String::from_utf8(bytes)
            .map_err(|_| invalid("messages.mdict_utf8_key", "Неправильный ключ UTF-8"))
    }
}

fn summary_key(reader: &mut impl Read, utf16: bool) -> Result<String> {
    let mut length = [0; 2];
    reader.read_exact(&mut length)?;
    let mut bytes = vec![0; u16::from_be_bytes(length) as usize * if utf16 { 2 } else { 1 }];
    reader.read_exact(&mut bytes)?;
    let mut zero = [0; 2];
    reader.read_exact(&mut zero[..if utf16 { 2 } else { 1 }])?;
    if zero != [0, 0] {
        return Err(invalid(
            "messages.mdict_key_terminator",
            "Нет терминатора в сводке ключей",
        ));
    }
    key_string(bytes, utf16)
}

fn row_key(reader: &mut impl Read, utf16: bool) -> Result<String> {
    let mut bytes = Vec::new();
    loop {
        let mut unit = [0; 2];
        reader.read_exact(&mut unit[..if utf16 { 2 } else { 1 }])?;
        if unit == [0, 0] {
            break;
        }
        bytes.extend_from_slice(&unit[..if utf16 { 2 } else { 1 }]);
        if bytes.len() > 2 * u16::MAX as usize {
            return Err(invalid(
                "messages.mdict_long_key",
                "Слишком длинный ключ MDict",
            ));
        }
    }
    let key = key_string(bytes, utf16)?;
    if key.is_empty() || key.contains('\0') {
        return Err(invalid(
            "messages.mdict_empty_key",
            "Пустой ключ или NUL внутри ключа",
        ));
    }
    Ok(key)
}

fn copy_index(
    reader: &mut impl Read,
    out: &mut impl Write,
    length: u64,
    ctx: &RunContext,
) -> Result<()> {
    let mut remaining = length;
    let mut buffer = [0; 64 * 1024];
    while remaining > 0 {
        ctx.control.check()?;
        let size = remaining.min(buffer.len() as u64) as usize;
        reader.read_exact(&mut buffer[..size])?;
        out.write_all(&buffer[..size])?;
        remaining -= size as u64;
    }
    Ok(())
}

fn validate_file(
    path: &std::path::Path,
    utf16: bool,
    entries: u64,
    offset: u64,
    total: u64,
    ctx: &RunContext,
) -> Result<()> {
    let mut file = File::open(path)?;
    let file_len = file.metadata()?.len();
    let mut header_len = [0; 4];
    file.read_exact(&mut header_len)?;
    let header_len = u32::from_be_bytes(header_len) as usize;
    if header_len > 32 * 1024 * 1024 || !header_len.is_multiple_of(2) {
        return Err(invalid(
            "messages.mdict_header_length",
            "Неверная длина заголовка MDict",
        ));
    }
    let mut xml = vec![0; header_len];
    file.read_exact(&mut xml)?;
    let mut hash = [0; 4];
    file.read_exact(&mut hash)?;
    if adler2::adler32_slice(&xml) != u32::from_le_bytes(hash) {
        return Err(invalid(
            "messages.mdict_corrupt_header",
            "Повреждён заголовок словаря",
        ));
    }
    let xml = key_string(xml, true)?;
    if !xml.contains("RequiredEngineVersion=\"2.0\"")
        || !xml.contains(if utf16 {
            "<Library_Data "
        } else {
            "<Dictionary "
        })
    {
        return Err(invalid(
            "messages.mdict_version",
            "Ожидался заголовок MDict v2",
        ));
    }
    let mut key_header = [0; 40];
    file.read_exact(&mut key_header)?;
    file.read_exact(&mut hash)?;
    if adler2::adler32_slice(&key_header) != u32::from_be_bytes(hash) {
        return Err(invalid(
            "messages.mdict_key_geometry",
            "Повреждена геометрия ключей",
        ));
    }
    let mut header = Cursor::new(key_header);
    let key_blocks = number(&mut header)?;
    let key_entries = number(&mut header)?;
    let index_raw = number(&mut header)?;
    let index_size = number(&mut header)?;
    let key_size = number(&mut header)?;
    if key_entries != entries
        || key_blocks > entries
        || key_blocks > key_size / 8
        || index_size > file_len
        || key_size > file_len
        || index_raw > ctx.limits.max_unpacked_bytes
    {
        return Err(invalid(
            "messages.mdict_key_sizes",
            "Неверные счётчики или размеры ключей",
        ));
    }
    let mut index = tempfile::NamedTempFile::new_in(&ctx.scratch_dir)?;
    block(&mut file, index_size, index_raw, &mut index, ctx)?;
    index.rewind()?;
    let mut index = BufReader::new(index);
    let mut counted = 0u64;
    let mut compressed = 0u64;
    let mut last_offset = 0;
    let mut previous_order: Option<String> = None;
    for _ in 0..key_blocks {
        ctx.control.check()?;
        let count = number(&mut index)?;
        let first = summary_key(&mut index, utf16)?;
        let last = summary_key(&mut index, utf16)?;
        let size = number(&mut index)?;
        let raw_size = number(&mut index)?;
        if count == 0
            || count > entries
            || raw_size > ctx.limits.key_block_bytes.max(2 * u16::MAX as usize + 10) as u64
        {
            return Err(invalid(
                "messages.mdict_key_summary",
                "Неверная сводка блока ключей",
            ));
        }
        let mut raw = Vec::new();
        block(&mut file, size, raw_size, &mut raw, ctx)?;
        let mut keys = Cursor::new(raw);
        for row in 0..count {
            let position = number(&mut keys)?;
            let key = row_key(&mut keys, utf16)?;
            if (row == 0 && key != first) || (row + 1 == count && key != last) {
                return Err(invalid(
                    "messages.mdict_summary_mismatch",
                    "Сводка не совпадает с ключами блока",
                ));
            }
            if position < last_offset || (counted == 0 && row == 0 && position != 0) {
                return Err(invalid(
                    "messages.mdict_record_offsets",
                    "Нарушен порядок смещений записей",
                ));
            }
            let order = if utf16 {
                mdd_sort_key(&key)
            } else {
                mdx_sort_key(&key)
            };
            if previous_order
                .as_ref()
                .is_some_and(|previous| previous > &order)
            {
                return Err(invalid(
                    "messages.mdict_unsorted_keys",
                    "Ключи MDict не отсортированы",
                ));
            }
            previous_order = Some(order);
            last_offset = position;
        }
        if keys.position() != raw_size {
            return Err(invalid(
                "messages.mdict_trailing_key_data",
                "Лишние данные в блоке ключей",
            ));
        }
        counted = sum(counted, count)?;
        compressed = sum(compressed, size)?;
        progress(ctx, sum(offset, file.stream_position()?)?, total, false);
    }
    let mut extra = [0];
    if index.read(&mut extra)? != 0 || counted != entries || compressed != key_size {
        return Err(invalid(
            "messages.mdict_index_summary",
            "Индекс ключей не совпадает со сводкой",
        ));
    }
    let records = number(&mut file)?;
    let record_entries = number(&mut file)?;
    let record_index_len = number(&mut file)?;
    let record_bytes = number(&mut file)?;
    if record_entries != entries
        || records.checked_mul(16) != Some(record_index_len)
        || records > record_bytes / 8
        || record_index_len > file_len
        || record_bytes > file_len
    {
        return Err(invalid(
            "messages.mdict_record_geometry",
            "Неверная геометрия записей",
        ));
    }
    let mut record_index = tempfile::NamedTempFile::new_in(&ctx.scratch_dir)?;
    copy_index(&mut file, &mut record_index, record_index_len, ctx)?;
    record_index.rewind()?;
    let mut record_index = BufReader::new(record_index);
    let mut raw_records = 0u64;
    let mut compressed_records = 0u64;
    for _ in 0..records {
        let size = number(&mut record_index)?;
        let raw = number(&mut record_index)?;
        if raw > ctx.limits.max_html_bytes.max(ctx.limits.record_block_bytes) as u64 {
            return Err(invalid(
                "messages.mdict_record_limit",
                "Блок записей превышает лимит",
            ));
        }
        block(&mut file, size, raw, &mut io::sink(), ctx)?;
        raw_records = sum(raw_records, raw)?;
        compressed_records = sum(compressed_records, size)?;
        progress(ctx, sum(offset, file.stream_position()?)?, total, false);
    }
    if last_offset > raw_records
        || compressed_records != record_bytes
        || file.stream_position()? != file_len
    {
        return Err(invalid(
            "messages.mdict_record_length",
            "Не совпала длина словаря или диапазон записей",
        ));
    }
    Ok(())
}

fn progress(ctx: &RunContext, completed: u64, total: u64, done: bool) {
    let value = StageProgress {
        stage: ConversionStage::Validate,
        completed,
        total: Some(total),
        unit: ProgressUnit::Bytes,
        articles: 0,
        resources: 0,
    };
    ctx.emit(if done {
        CoreEvent::StageCompleted(value)
    } else {
        CoreEvent::Progress(value)
    });
}

pub fn validate_artifacts(artifacts: &ConvertedArtifacts, ctx: &RunContext) -> Result<()> {
    let result = (|| {
        ctx.control.check()?;
        let mdx_size = std::fs::metadata(&artifacts.mdx)?.len();
        let mdd_size = std::fs::metadata(&artifacts.mdd)?.len();
        let total = sum(mdx_size, mdd_size)?;
        validate_file(
            &artifacts.mdx,
            false,
            artifacts.summary.output_keys,
            0,
            total,
            ctx,
        )?;
        validate_file(
            &artifacts.mdd,
            true,
            artifacts.summary.resources,
            mdx_size,
            total,
            ctx,
        )?;
        progress(ctx, total, total, true);
        Ok(())
    })();
    result.map_err(|mut error: ConverterError| {
        if error.code == "cancelled" {
            error
        } else {
            error.code = "invalid_mdict".into();
            error
        }
    })
}
