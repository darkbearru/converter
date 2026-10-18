use std::fs::File;
use std::io::{self, Cursor, Seek, SeekFrom, Write};

use converter_core::mdict::{write_mdd, write_mdx};
use converter_core::model::{DictionaryMetadata, MdxRecord, ResourceRecord, RunContext};
use mdictlib::{ChecksumPolicy, MddFile, MdxFile, OpenOptions};
use sha2::{Digest, Sha256};

fn metadata() -> DictionaryMetadata {
    DictionaryMetadata {
        title: "Русский & English <словарь>".into(),
        ..Default::default()
    }
}

fn records() -> Vec<MdxRecord> {
    [
        ("a-b", "<p>Дефис</p>"),
        ("ab", "<p>Без дефиса</p>"),
        ("Alpha", "<p>Первый</p>"),
        ("яблоко", "<p>Первое значение</p>"),
        ("яблоко", "<p>Второе значение</p>"),
        ("你好", "<p>Привет</p>"),
    ]
    .into_iter()
    .map(|(key, html)| MdxRecord {
        key: key.into(),
        html: html.into(),
    })
    .collect()
}

fn reader_options() -> OpenOptions {
    OpenOptions::new().with_checksum_policy(ChecksumPolicy::Verify)
}

#[test]
fn пишет_совместимый_mdx_и_сохраняет_дубликаты() -> Result<(), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("словарь.mdx");
    let mut ctx = RunContext::new(folder.path().to_owned());
    ctx.limits.key_block_bytes = 24;
    ctx.limits.record_block_bytes = 48;
    let expected = records();
    let summary = write_mdx(
        expected.clone().into_iter().map(Ok),
        &metadata(),
        File::create(&path)?,
        &ctx,
    )?;
    assert_eq!(summary.entries, 6);
    assert!(summary.key_blocks > 1);
    assert!(summary.record_blocks > 1);

    let dictionary = MdxFile::open_with_options(&path, &reader_options())?;
    assert_eq!(dictionary.header().title(), Some(metadata().title.as_str()));
    let mut decoded = Vec::new();
    for key in dictionary.keys() {
        let key = key?;
        let entry = dictionary
            .entry_at(key.ordinal())?
            .ok_or("Не найдена статья")?;
        decoded.push((key.key().to_owned(), entry.text().to_owned()));
    }
    assert_eq!(
        decoded,
        expected
            .into_iter()
            .map(|v| (v.key, v.html))
            .collect::<Vec<_>>()
    );
    assert_eq!(decoded.iter().filter(|(key, _)| key == "яблоко").count(), 2);
    for (key, body) in [
        ("a-b", "Дефис"),
        ("ab", "Без дефиса"),
        ("Alpha", "Первый"),
        ("你好", "Привет"),
    ] {
        assert!(
            dictionary
                .lookup(key)?
                .ok_or("Не найден ключ")?
                .text()
                .contains(body)
        );
    }
    assert!(dictionary.lookup("a b")?.is_some());
    Ok(())
}

#[test]
fn передает_большой_ресурс_через_границы_блоков() -> Result<(), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let resource = folder.path().join("resource.bin");
    let original: Vec<u8> = (0..1_200_003).map(|n| (n % 251) as u8).collect();
    std::fs::write(&resource, &original)?;
    let target = folder.path().join("ресурсы.mdd");
    let mut ctx = RunContext::new(folder.path().to_owned());
    ctx.limits.record_block_bytes = 4096;
    let summary = write_mdd(
        [Ok(ResourceRecord {
            key: "\\media\\голос.bin".into(),
            path: resource,
        })]
        .into_iter(),
        File::create(&target)?,
        &ctx,
    )?;
    assert_eq!(summary.entries, 1);
    assert!(summary.record_blocks > 200);
    let dictionary = MddFile::open_with_options(&target, &reader_options())?;
    let span = dictionary
        .lookup_span("\\media\\голос.bin")?
        .ok_or("Не найден ресурс")?;
    let mut copied = Vec::new();
    assert_eq!(span.copy_to(&mut copied)?, original.len() as u64);
    assert_eq!(Sha256::digest(&copied), Sha256::digest(&original));
    assert_eq!(copied, original);
    Ok(())
}

#[test]
fn отклоняет_нарушенный_порядок_и_nul_в_ключе() -> Result<(), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let ctx = RunContext::new(folder.path().to_owned());
    let mut output = Cursor::new(Vec::new());
    let invalid = [
        MdxRecord {
            key: "z".into(),
            html: "z".into(),
        },
        MdxRecord {
            key: "a".into(),
            html: "a".into(),
        },
    ];
    assert_eq!(
        write_mdx(invalid.into_iter().map(Ok), &metadata(), &mut output, &ctx)
            .expect_err("Нужна ошибка порядка")
            .code,
        "unsorted_keys"
    );
    assert!(output.into_inner().is_empty());
    let bad = MdxRecord {
        key: "a\0b".into(),
        html: "text".into(),
    };
    assert_eq!(
        write_mdx(
            [Ok(bad)].into_iter(),
            &metadata(),
            Cursor::new(Vec::new()),
            &ctx
        )
        .expect_err("NUL недопустим")
        .code,
        "invalid_key"
    );
    Ok(())
}

#[test]
fn отмена_и_сбой_записи_не_возвращают_успех() -> Result<(), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let ctx = RunContext::new(folder.path().to_owned());
    ctx.control.cancel();
    assert_eq!(
        write_mdx(
            records().into_iter().map(Ok),
            &metadata(),
            Cursor::new(Vec::new()),
            &ctx
        )
        .expect_err("Нужна отмена")
        .code,
        "cancelled"
    );
    let ctx = RunContext::new(folder.path().to_owned());
    let failure = FailingWriter(Cursor::new(Vec::new()));
    assert_eq!(
        write_mdx(records().into_iter().map(Ok), &metadata(), failure, &ctx)
            .expect_err("Нужна ошибка записи")
            .code,
        "io_error"
    );
    Ok(())
}

struct FailingWriter(Cursor<Vec<u8>>);
impl Write for FailingWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("Контролируемый сбой"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Seek for FailingWriter {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.0.seek(position)
    }
}
