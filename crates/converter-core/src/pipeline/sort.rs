use crate::{mdict::collation::mdx_sort_key, model::*};
use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    fs::File,
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

#[derive(Debug)]
struct Row {
    key: String,
    order: String,
    ordinal: u64,
    offset: u64,
    length: u64,
}

impl Row {
    fn heap_bytes(&self) -> usize {
        self.key.capacity() + self.order.capacity()
    }
    fn compare(&self, other: &Self) -> Ordering {
        self.order
            .cmp(&other.order)
            .then(self.ordinal.cmp(&other.ordinal))
    }
    fn write(&self, out: &mut impl Write) -> Result<()> {
        out.write_all(&(self.key.len() as u32).to_be_bytes())?;
        out.write_all(&(self.order.len() as u32).to_be_bytes())?;
        for number in [self.ordinal, self.offset, self.length] {
            out.write_all(&number.to_be_bytes())?;
        }
        out.write_all(self.key.as_bytes())?;
        out.write_all(self.order.as_bytes())?;
        Ok(())
    }
}

fn corrupt() -> ConverterError {
    ConverterError::new("corrupt_spool", "Повреждён промежуточный файл сортировки")
}
fn budget_error() -> ConverterError {
    ConverterError::new(
        "sort_budget_too_small",
        "Бюджет сортировки слишком мал для ключей или выбранного числа потоков объединения",
    )
}

fn read_row(reader: &mut impl Read, ctx: &RunContext) -> Result<Option<Row>> {
    ctx.control.check()?;
    let mut header = [0; 32];
    if reader.read(&mut header[..1])? == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut header[1..])?;
    let key_len = u32::from_be_bytes(header[..4].try_into().map_err(|_| corrupt())?) as usize;
    let order_len = u32::from_be_bytes(header[4..8].try_into().map_err(|_| corrupt())?) as usize;
    let ordinal = u64::from_be_bytes(header[8..16].try_into().map_err(|_| corrupt())?);
    let offset = u64::from_be_bytes(header[16..24].try_into().map_err(|_| corrupt())?);
    let length = u64::from_be_bytes(header[24..32].try_into().map_err(|_| corrupt())?);
    if key_len > u16::MAX as usize
        || order_len > 3 * u16::MAX as usize
        || length > ctx.limits.max_html_bytes as u64
    {
        return Err(corrupt());
    }
    let mut key = vec![0; key_len];
    let mut order = vec![0; order_len];
    reader.read_exact(&mut key)?;
    reader.read_exact(&mut order)?;
    Ok(Some(Row {
        key: String::from_utf8(key).map_err(|_| corrupt())?,
        order: String::from_utf8(order).map_err(|_| corrupt())?,
        ordinal,
        offset,
        length,
    }))
}

pub(super) struct ExternalSort {
    directory: PathBuf,
    bodies: PathBuf,
    ctx: RunContext,
    rows: Vec<Row>,
    heap_bytes: usize,
    runs: u64,
    entries: u64,
    pub metrics: SortMetrics,
}

impl ExternalSort {
    pub fn new(directory: &Path, bodies: &Path, ctx: &RunContext) -> Result<Self> {
        if ctx.limits.sort_buffer_bytes < 256 || ctx.limits.merge_fan_in < 2 {
            return Err(budget_error());
        }
        std::fs::create_dir(directory)?;
        Ok(Self {
            directory: directory.into(),
            bodies: bodies.into(),
            ctx: ctx.clone(),
            rows: Vec::new(),
            heap_bytes: 0,
            runs: 0,
            entries: 0,
            metrics: SortMetrics::default(),
        })
    }

    fn path(&self, pass: u32, index: u64) -> PathBuf {
        self.directory.join(format!("run-{pass}-{index}"))
    }

    pub fn push(&mut self, key: String, offset: u64, length: u64) -> Result<()> {
        self.ctx.control.check()?;
        if key.is_empty() || key.contains('\0') || key.len() > u16::MAX as usize {
            return Err(ConverterError::new(
                "invalid_key",
                "Недопустимый поисковый ключ",
            ));
        }
        let row = Row {
            order: mdx_sort_key(&key),
            key,
            ordinal: self.entries,
            offset,
            length,
        };
        let row_size = row.heap_bytes().saturating_add(size_of::<Row>());
        if row_size > self.ctx.limits.sort_buffer_bytes {
            return Err(budget_error());
        }
        let used = self.rows.capacity() * size_of::<Row>() + self.heap_bytes;
        if self.rows.len() == self.rows.capacity()
            || used.saturating_add(row.heap_bytes()) > self.ctx.limits.sort_buffer_bytes
        {
            self.flush()?;
        }
        if self.rows.capacity() == 0 {
            let capacity = (self.ctx.limits.sort_buffer_bytes / row_size).max(1);
            self.rows = Vec::with_capacity(capacity);
        }
        let used = self.rows.capacity() * size_of::<Row>() + self.heap_bytes + row.heap_bytes();
        if used > self.ctx.limits.sort_buffer_bytes {
            // Ключ шире предыдущих: новый run получает меньшую таблицу строк.
            self.rows = Vec::with_capacity(1);
        }
        self.heap_bytes += row.heap_bytes();
        self.rows.push(row);
        let used = self.rows.capacity() * size_of::<Row>() + self.heap_bytes;
        if used > self.ctx.limits.sort_buffer_bytes {
            return Err(budget_error());
        }
        self.metrics.peak_sort_bytes = self.metrics.peak_sort_bytes.max(used);
        self.entries = self.entries.checked_add(1).ok_or_else(corrupt)?;
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        if self.rows.is_empty() {
            return Ok(());
        }
        self.ctx.control.check()?;
        self.rows.sort_unstable_by(Row::compare);
        let mut file = BufWriter::new(
            File::options()
                .write(true)
                .create_new(true)
                .open(self.path(0, self.runs))?,
        );
        for row in &self.rows {
            self.ctx.control.check()?;
            row.write(&mut file)?;
        }
        file.flush()?;
        self.runs += 1;
        self.rows = Vec::new();
        self.heap_bytes = 0;
        Ok(())
    }

    pub fn finish(&mut self) -> Result<SortedRecords> {
        self.flush()?;
        if self.entries == 0 {
            return Err(ConverterError::new(
                "empty_dictionary",
                "Нет поисковых ключей для записи",
            ));
        }
        let fan_in = self.ctx.limits.merge_fan_in as u64;
        let mut count = self.runs;
        let mut passes = 0u32;
        while count > 1 {
            count = count.div_ceil(fan_in);
            passes += 1;
        }
        let total = self
            .entries
            .checked_mul(u64::from(passes) + 1)
            .ok_or_else(corrupt)?;
        let mut completed = self.entries;
        self.progress(completed, total, false);
        self.ctx.control.check()?;
        count = self.runs;
        let mut pass = 0;
        while count > 1 {
            let next_count = count.div_ceil(fan_in);
            for group in 0..next_count {
                self.ctx.control.check()?;
                let first = group * fan_in;
                let end = count.min(first + fan_in);
                let paths: Vec<_> = (first..end).map(|index| self.path(pass, index)).collect();
                let mut out = BufWriter::new(
                    File::options()
                        .write(true)
                        .create_new(true)
                        .open(self.path(pass + 1, group))?,
                );
                let mut heap = BinaryHeap::with_capacity(paths.len());
                let mut readers = Vec::with_capacity(paths.len());
                let mut heap_bytes = 0usize;
                for (index, path) in paths.iter().enumerate() {
                    let mut reader = BufReader::new(File::open(path)?);
                    if let Some(row) = read_row(&mut reader, &self.ctx)? {
                        heap_bytes += row.heap_bytes();
                        heap.push(HeapRow { row, index });
                    }
                    readers.push(reader);
                }
                while let Some(HeapRow { row, index }) = heap.pop() {
                    self.ctx.control.check()?;
                    let used = heap.capacity() * size_of::<HeapRow>() + heap_bytes;
                    if used > self.ctx.limits.sort_buffer_bytes {
                        return Err(budget_error());
                    }
                    self.metrics.peak_sort_bytes = self.metrics.peak_sort_bytes.max(used);
                    row.write(&mut out)?;
                    heap_bytes -= row.heap_bytes();
                    drop(row);
                    completed += 1;
                    if completed.is_multiple_of(1024) {
                        self.progress(completed, total, false);
                    }
                    if let Some(row) = read_row(&mut readers[index], &self.ctx)? {
                        heap_bytes += row.heap_bytes();
                        heap.push(HeapRow { row, index });
                    }
                }
                out.flush()?;
                drop(out);
                drop(readers);
                for path in paths {
                    std::fs::remove_file(path)?;
                }
            }
            count = next_count;
            pass += 1;
        }
        self.metrics.merge_passes = pass;
        self.progress(completed, total, true);
        self.ctx.control.check()?;
        Ok(SortedRecords {
            reader: BufReader::new(File::open(self.path(pass, 0))?),
            bodies: File::open(&self.bodies)?,
            remaining: self.entries,
            failed: false,
            ctx: self.ctx.clone(),
        })
    }

    fn progress(&self, completed: u64, total: u64, done: bool) {
        let progress = StageProgress {
            stage: ConversionStage::Sort,
            completed,
            total: Some(total),
            unit: ProgressUnit::Entries,
            articles: 0,
            resources: 0,
        };
        self.ctx.emit(if done {
            CoreEvent::StageCompleted(progress)
        } else {
            CoreEvent::Progress(progress)
        });
    }
}

struct HeapRow {
    row: Row,
    index: usize,
}
impl PartialEq for HeapRow {
    fn eq(&self, other: &Self) -> bool {
        self.row.compare(&other.row) == Ordering::Equal
    }
}
impl Eq for HeapRow {}
impl PartialOrd for HeapRow {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HeapRow {
    fn cmp(&self, other: &Self) -> Ordering {
        other.row.compare(&self.row)
    }
}

pub(super) struct SortedRecords {
    reader: BufReader<File>,
    bodies: File,
    remaining: u64,
    failed: bool,
    ctx: RunContext,
}
impl Iterator for SortedRecords {
    type Item = Result<MdxRecord>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.remaining == 0 {
            return None;
        }
        let result = (|| {
            let row = read_row(&mut self.reader, &self.ctx)?.ok_or_else(corrupt)?;
            let mut body = vec![0; usize::try_from(row.length).map_err(|_| corrupt())?];
            self.bodies.seek(SeekFrom::Start(row.offset))?;
            self.bodies.read_exact(&mut body)?;
            Ok(MdxRecord {
                key: row.key,
                html: String::from_utf8(body).map_err(|_| corrupt())?,
            })
        })();
        self.remaining -= 1;
        self.failed = result.is_err();
        Some(result)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.failed {
            return (0, Some(0));
        }
        match usize::try_from(self.remaining) {
            Ok(size) => (size, Some(size)),
            Err(_) => (usize::MAX, None),
        }
    }
}
