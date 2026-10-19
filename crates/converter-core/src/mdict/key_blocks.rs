use super::{
    blocks::{add, envelope, temporary},
    collation::{mdd_sort_key, mdx_sort_key},
};
use crate::model::{ConverterError, Result, RunContext};
use std::io::Write;
use tempfile::NamedTempFile;

pub(super) struct KeyBlocks {
    pub data: NamedTempFile,
    pub index: NamedTempFile,
    pub blocks: u64,
    pub entries: u64,
    pub bytes: u64,
    utf16: bool,
    limit: usize,
    buffer: Vec<u8>,
    first: Vec<u8>,
    last: Vec<u8>,
    block_entries: u64,
    previous: Option<String>,
}

impl KeyBlocks {
    pub fn new(utf16: bool, ctx: &RunContext) -> Result<Self> {
        Ok(Self {
            data: temporary(ctx)?,
            index: temporary(ctx)?,
            blocks: 0,
            entries: 0,
            bytes: 0,
            utf16,
            limit: ctx.limits.key_block_bytes,
            buffer: Vec::new(),
            first: Vec::new(),
            last: Vec::new(),
            block_entries: 0,
            previous: None,
        })
    }

    pub fn push(&mut self, key: &str, offset: u64) -> Result<()> {
        if key.is_empty() || key.contains('\0') {
            return Err(
                ConverterError::new("invalid_key", "Ключ словаря пуст или содержит NUL")
                    .with_message("messages.invalid_key_2", crate::model::message_args([])),
            );
        }
        let order = if self.utf16 {
            mdd_sort_key(key)
        } else {
            mdx_sort_key(key)
        };
        if self.previous.as_ref().is_some_and(|last| *last > order) {
            return Err(ConverterError::new(
                "unsorted_keys",
                format!("Нарушен порядок ключей: {key}"),
            )
            .with_message(
                "messages.unsorted_keys",
                crate::model::message_args([("key", key.to_string())]),
            ));
        }
        let encoded: Vec<u8> = if self.utf16 {
            key.encode_utf16().flat_map(u16::to_le_bytes).collect()
        } else {
            key.as_bytes().to_vec()
        };
        let units = encoded.len() / if self.utf16 { 2 } else { 1 };
        if units > u16::MAX as usize {
            return Err(ConverterError::new(
                "invalid_key",
                "Ключ превышает длину 65535 единиц формата",
            )
            .with_message("messages.invalid_key_3", crate::model::message_args([])));
        }
        let terminator = if self.utf16 { 2 } else { 1 };
        if !self.buffer.is_empty()
            && self
                .buffer
                .len()
                .saturating_add(8 + encoded.len() + terminator)
                > self.limit
        {
            self.flush()?;
        }
        if self.block_entries == 0 {
            self.first.clone_from(&encoded);
        }
        self.last.clone_from(&encoded);
        self.buffer.extend_from_slice(&offset.to_be_bytes());
        self.buffer.extend_from_slice(&encoded);
        self.buffer.extend_from_slice(&[0; 2][..terminator]);
        self.block_entries = add(self.block_entries, 1)?;
        self.entries = add(self.entries, 1)?;
        self.previous = Some(order);
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        if self.block_entries == 0 {
            return Ok(());
        }
        let size = envelope(&self.buffer, &mut self.data)?;
        self.index.write_all(&self.block_entries.to_be_bytes())?;
        for key in [&self.first, &self.last] {
            let units = key.len() / if self.utf16 { 2 } else { 1 };
            self.index.write_all(&(units as u16).to_be_bytes())?;
            self.index.write_all(key)?;
            self.index
                .write_all(if self.utf16 { &[0, 0] } else { &[0] })?;
        }
        self.index.write_all(&size.to_be_bytes())?;
        self.index
            .write_all(&(self.buffer.len() as u64).to_be_bytes())?;
        self.blocks = add(self.blocks, 1)?;
        self.bytes = add(self.bytes, size)?;
        self.buffer.clear();
        self.block_entries = 0;
        Ok(())
    }
}
