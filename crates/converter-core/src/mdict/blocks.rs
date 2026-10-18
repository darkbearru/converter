use crate::model::{ConverterError, Result, RunContext};
use flate2::{Compression, write::ZlibEncoder};
use std::io::{Read, Seek, SeekFrom, Write};
use tempfile::NamedTempFile;

pub(super) fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(|| {
        ConverterError::new(
            "size_overflow",
            "Размер словаря превышает возможности формата",
        )
    })
}

pub(super) fn temporary(ctx: &RunContext) -> Result<NamedTempFile> {
    Ok(NamedTempFile::new_in(&ctx.scratch_dir)?)
}

pub(super) fn envelope(payload: &[u8], target: &mut impl Write) -> Result<u64> {
    let mut compressor = ZlibEncoder::new(Vec::new(), Compression::default());
    compressor.write_all(payload)?;
    let compressed = compressor.finish()?;
    target.write_all(&2u32.to_le_bytes())?;
    target.write_all(&adler2::adler32_slice(payload).to_be_bytes())?;
    target.write_all(&compressed)?;
    add(compressed.len() as u64, 8)
}

pub(super) fn copy(
    source: &mut impl Read,
    target: &mut impl Write,
    ctx: &RunContext,
) -> Result<u64> {
    let mut buffer = vec![0; ctx.limits.resource_buffer_bytes.clamp(1, 1024 * 1024)];
    let mut size = 0;
    loop {
        ctx.control.check()?;
        let count = source.read(&mut buffer)?;
        if count == 0 {
            return Ok(size);
        }
        target.write_all(&buffer[..count])?;
        size = add(size, count as u64)?;
    }
}

pub(super) fn compress_index(
    source: &mut NamedTempFile,
    ctx: &RunContext,
) -> Result<NamedTempFile> {
    let mut target = temporary(ctx)?;
    target.write_all(&2u32.to_le_bytes())?;
    target.write_all(&[0; 4])?;
    source.rewind()?;
    let mut checksum = adler2::Adler32::new();
    {
        let mut compressor = ZlibEncoder::new(target.as_file_mut(), Compression::default());
        let mut buffer = [0; 64 * 1024];
        loop {
            ctx.control.check()?;
            let size = source.read(&mut buffer)?;
            if size == 0 {
                break;
            }
            checksum.write_slice(&buffer[..size]);
            compressor.write_all(&buffer[..size])?;
        }
        compressor.finish()?;
    }
    target.seek(SeekFrom::Start(4))?;
    target.write_all(&checksum.checksum().to_be_bytes())?;
    target.rewind()?;
    Ok(target)
}
