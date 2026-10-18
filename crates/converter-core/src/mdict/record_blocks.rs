use super::blocks::{add, envelope, temporary};
use crate::model::{Result, RunContext};
use std::io::Write;
use tempfile::NamedTempFile;

pub(super) struct RecordBlocks {
    pub data: NamedTempFile,
    pub index: NamedTempFile,
    pub blocks: u64,
    pub raw_bytes: u64,
    pub bytes: u64,
    buffer: Vec<u8>,
    limit: usize,
}

impl RecordBlocks {
    pub fn new(ctx: &RunContext) -> Result<Self> {
        Ok(Self {
            data: temporary(ctx)?,
            index: temporary(ctx)?,
            blocks: 0,
            raw_bytes: 0,
            bytes: 0,
            buffer: Vec::new(),
            limit: ctx.limits.record_block_bytes,
        })
    }

    /// Статья MDX остаётся внутри одного блока; ресурс MDD может пересекать блоки.
    pub fn article(&mut self, body: &[u8]) -> Result<()> {
        if !self.buffer.is_empty() && self.buffer.len().saturating_add(body.len()) > self.limit {
            self.flush()?;
        }
        self.buffer.extend_from_slice(body);
        self.raw_bytes = add(self.raw_bytes, body.len() as u64)?;
        Ok(())
    }

    pub fn resource_chunk(&mut self, mut chunk: &[u8]) -> Result<()> {
        self.raw_bytes = add(self.raw_bytes, chunk.len() as u64)?;
        while !chunk.is_empty() {
            let size = (self.limit - self.buffer.len()).min(chunk.len());
            self.buffer.extend_from_slice(&chunk[..size]);
            chunk = &chunk[size..];
            if self.buffer.len() == self.limit {
                self.flush()?;
            }
        }
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let size = envelope(&self.buffer, &mut self.data)?;
        self.index.write_all(&size.to_be_bytes())?;
        self.index
            .write_all(&(self.buffer.len() as u64).to_be_bytes())?;
        self.blocks = add(self.blocks, 1)?;
        self.bytes = add(self.bytes, size)?;
        self.buffer.clear();
        Ok(())
    }
}
