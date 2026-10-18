use super::{
    blocks::{add, compress_index, copy},
    header::write_header,
    key_blocks::KeyBlocks,
    record_blocks::RecordBlocks,
};
use crate::model::*;
use std::{
    fs::File,
    io::{Read, Seek, Write},
};

fn validate_limits(ctx: &RunContext) -> Result<()> {
    ctx.control.check()?;
    if ctx.limits.key_block_bytes == 0
        || ctx.limits.record_block_bytes == 0
        || ctx.limits.resource_buffer_bytes == 0
    {
        return Err(ConverterError::new(
            "invalid_limits",
            "Размеры рабочих буферов должны быть положительными",
        ));
    }
    std::fs::create_dir_all(&ctx.scratch_dir)?;
    Ok(())
}

pub fn write_mdx<W: Write + Seek>(
    records: impl Iterator<Item = Result<MdxRecord>>,
    metadata: &DictionaryMetadata,
    out: W,
    ctx: &RunContext,
) -> Result<WriteSummary> {
    validate_limits(ctx)?;
    let mut keys = KeyBlocks::new(false, ctx)?;
    let mut bodies = RecordBlocks::new(ctx)?;
    let hint = records.size_hint();
    let total = (Some(hint.0) == hint.1).then_some(hint.0 as u64);
    for record in records {
        ctx.control.check()?;
        let record = record?;
        if record.html.len() > ctx.limits.max_html_bytes {
            return Err(ConverterError::new(
                "article_too_large",
                "HTML статьи превышает допустимый размер",
            ));
        }
        keys.push(&record.key, bodies.raw_bytes)?;
        bodies.article(record.html.as_bytes())?;
        progress(
            ctx,
            ConversionStage::WriteMdx,
            keys.entries,
            total,
            ProgressUnit::Entries,
            keys.entries,
            0,
            false,
        );
    }
    let summary = finish(keys, bodies, Some(metadata), out, ctx)?;
    progress(
        ctx,
        ConversionStage::WriteMdx,
        summary.entries,
        Some(summary.entries),
        ProgressUnit::Entries,
        summary.entries,
        0,
        true,
    );
    Ok(summary)
}

pub fn write_mdd<W: Write + Seek>(
    resources: impl Iterator<Item = Result<ResourceRecord>>,
    out: W,
    ctx: &RunContext,
) -> Result<WriteSummary> {
    validate_limits(ctx)?;
    let mut keys = KeyBlocks::new(true, ctx)?;
    let mut bodies = RecordBlocks::new(ctx)?;
    let mut buffer = vec![0; ctx.limits.resource_buffer_bytes.min(1024 * 1024)];
    for resource in resources {
        ctx.control.check()?;
        let resource = resource?;
        keys.push(&resource.key, bodies.raw_bytes)?;
        let mut file = File::open(&resource.path)?;
        loop {
            ctx.control.check()?;
            let size = file.read(&mut buffer)?;
            if size == 0 {
                break;
            }
            bodies.resource_chunk(&buffer[..size])?;
            progress(
                ctx,
                ConversionStage::WriteMdd,
                bodies.raw_bytes,
                None,
                ProgressUnit::Bytes,
                0,
                keys.entries - 1,
                false,
            );
        }
    }
    let bytes = bodies.raw_bytes;
    let summary = finish(keys, bodies, None, out, ctx)?;
    progress(
        ctx,
        ConversionStage::WriteMdd,
        bytes,
        Some(bytes),
        ProgressUnit::Bytes,
        0,
        summary.entries,
        true,
    );
    Ok(summary)
}

#[allow(clippy::too_many_arguments)]
fn progress(
    ctx: &RunContext,
    stage: ConversionStage,
    completed: u64,
    total: Option<u64>,
    unit: ProgressUnit,
    articles: u64,
    resources: u64,
    done: bool,
) {
    let event = StageProgress {
        stage,
        completed,
        total,
        unit,
        articles,
        resources,
    };
    ctx.emit(if done {
        CoreEvent::StageCompleted(event)
    } else {
        CoreEvent::Progress(event)
    });
}

fn finish<W: Write + Seek>(
    mut keys: KeyBlocks,
    mut bodies: RecordBlocks,
    metadata: Option<&DictionaryMetadata>,
    mut out: W,
    ctx: &RunContext,
) -> Result<WriteSummary> {
    ctx.control.check()?;
    keys.flush()?;
    bodies.flush()?;
    let raw_index_size = keys.index.as_file().metadata()?.len();
    let mut index = compress_index(&mut keys.index, ctx)?;
    ctx.control.check()?;
    // До этого места целевой файл не изменяется: неверный вход не даёт частичного MDX.
    let start = out.stream_position()?;
    write_header(&mut out, metadata)?;
    let mut header = Vec::with_capacity(40);
    for number in [
        keys.blocks,
        keys.entries,
        raw_index_size,
        index.as_file().metadata()?.len(),
        keys.bytes,
    ] {
        header.extend_from_slice(&number.to_be_bytes());
    }
    out.write_all(&header)?;
    out.write_all(&adler2::adler32_slice(&header).to_be_bytes())?;
    copy(&mut index, &mut out, ctx)?;
    keys.data.rewind()?;
    copy(&mut keys.data, &mut out, ctx)?;
    let index_size = bodies.blocks.checked_mul(16).ok_or_else(|| {
        ConverterError::new("size_overflow", "Переполнение размера индекса записей")
    })?;
    for number in [bodies.blocks, keys.entries, index_size, bodies.bytes] {
        out.write_all(&number.to_be_bytes())?;
    }
    bodies.index.rewind()?;
    copy(&mut bodies.index, &mut out, ctx)?;
    bodies.data.rewind()?;
    copy(&mut bodies.data, &mut out, ctx)?;
    ctx.control.check()?;
    out.flush()?;
    let total_bytes = out
        .stream_position()?
        .checked_sub(start)
        .ok_or_else(|| ConverterError::new("size_overflow", "Некорректное смещение результата"))?;
    let _ = add(keys.bytes, bodies.bytes)?;
    Ok(WriteSummary {
        entries: keys.entries,
        key_blocks: keys.blocks,
        record_blocks: bodies.blocks,
        total_bytes,
    })
}
