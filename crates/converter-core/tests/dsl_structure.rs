mod support;

use converter_core::{
    dsl::{
        DslReader,
        decoder::{decode_lines, resolve_encoding},
    },
    formats::DictionaryReader,
    model::{EncodingChoice, InputBundle, PreparedFile, ReadOptions, RunContext},
};
use encoding_rs::{Encoding, UTF_8, UTF_16BE, UTF_16LE, WINDOWS_1251};
use std::{
    fs::File,
    io::{Cursor, Read, Write},
    path::Path,
};
use support::collect_sink::CollectSink;

const DSL: &str = "#NAME \"Тестовый словарь\"\r\n#INDEX_LANGUAGE \"Russian\"\r\n#CONTENTS_LANGUAGE \"English\"\r\n{{многострочный\r\nкомментарий}}\r\nяблоко\r\napple\r\n Первый текст\r\n @росток\r\n  Дочерняя статья\r\n @\r\ntwo\r\n Текст с \u{feff} внутри\r\n";

fn bundle(path: &Path) -> InputBundle {
    InputBundle {
        annotation: None,
        includes: vec![],
        main: PreparedFile {
            id: "main".into(),
            display_name: path
                .file_name()
                .expect("Имя файла")
                .to_string_lossy()
                .into_owned(),
            path: path.into(),
        },
        abbreviations: None,
        resources: vec![],
    }
}

fn encoded(text: &str, encoding: &'static Encoding, bom: bool) -> Vec<u8> {
    if encoding == UTF_16LE || encoding == UTF_16BE {
        let mut bytes = if bom {
            if encoding == UTF_16LE {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            }
        } else {
            vec![]
        };
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&if encoding == UTF_16LE {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        }
        bytes
    } else {
        let mut bytes = if bom { vec![0xef, 0xbb, 0xbf] } else { vec![] };
        bytes.extend_from_slice(&encoding.encode(text).0);
        bytes
    }
}

#[test]
fn разбирает_кодировки_и_границы_буфера() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    // Windows-1251 не содержит BOM внутри текста, поэтому проверяем её отдельным вариантом.
    for (index, (encoding, bom)) in [
        (UTF_8, false),
        (UTF_8, true),
        (UTF_16LE, true),
        (UTF_16BE, true),
        (WINDOWS_1251, false),
    ]
    .into_iter()
    .enumerate()
    {
        let text = if encoding == WINDOWS_1251 {
            DSL.replace('\u{feff}', "")
        } else {
            DSL.to_owned()
        };
        let bytes = encoded(&text, encoding, bom);
        let path = temp.path().join(format!("source-{index}.dsl"));
        std::fs::write(&path, &bytes)?;
        let ctx = RunContext::new(temp.path().join("work"));
        let metadata = DslReader.inspect(&bundle(&path), &ReadOptions::default(), &ctx)?;
        assert_eq!(metadata.title, "Тестовый словарь");
        let mut sink = CollectSink::default();
        let summary = DslReader.read(&bundle(&path), &ReadOptions::default(), &mut sink, &ctx)?;
        assert_eq!(summary.source_articles, 3);
        assert_eq!(sink.entries[0].headwords, ["яблоко", "apple"]);
        assert_eq!(sink.entries[0].source_line, 6);
        assert_eq!(sink.entries[1].parent_id, Some(sink.entries[0].id));
        assert!(sink.diagnostics.is_empty());
        if encoding != WINDOWS_1251 {
            assert!(sink.entries[2].html.contains('\u{feff}'));
        }
        for chunk in 1..=7 {
            let mut lines = Vec::new();
            decode_lines(
                Chunks {
                    inner: Cursor::new(&bytes),
                    size: chunk,
                },
                encoding,
                &ctx,
                |line| {
                    lines.push(line.text);
                    Ok(true)
                },
            )?;
            assert_eq!(
                lines,
                text.lines().map(str::to_owned).collect::<Vec<_>>(),
                "{}, порция {chunk}",
                encoding.name()
            );
        }
    }
    let path = temp.path().join("source.dsl.dz");
    let mut encoder =
        flate2::write::GzEncoder::new(File::create(&path)?, flate2::Compression::default());
    encoder.write_all(DSL.as_bytes())?;
    encoder.finish()?;
    let mut sink = CollectSink::default();
    DslReader.read(
        &bundle(&path),
        &ReadOptions::default(),
        &mut sink,
        &RunContext::new(temp.path().join("dz")),
    )?;
    assert_eq!(sink.entries.len(), 3);
    Ok(())
}

#[test]
fn проверяет_весь_источник_до_выбора_кодировки() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("long.dsl");
    let text = format!(
        "{{{{{}}}}}\n#NAME \"Кириллица\"\nслово\n значение\n",
        "a".repeat(70_000)
    );
    std::fs::write(&path, encoded(&text, WINDOWS_1251, false))?;
    let ctx = RunContext::new(temp.path().join("work"));
    assert_eq!(
        resolve_encoding(&bundle(&path).main, EncodingChoice::Auto, &ctx)?.encoding_name,
        "windows-1251"
    );
    assert_eq!(
        DslReader
            .inspect(&bundle(&path), &ReadOptions::default(), &ctx)?
            .title,
        "Кириллица"
    );
    std::fs::write(&path, encoded(DSL, UTF_16LE, false))?;
    assert_eq!(
        resolve_encoding(&bundle(&path).main, EncodingChoice::Auto, &ctx)
            .expect_err("Нужна ручная кодировка")
            .code,
        "encoding_required"
    );
    let options = ReadOptions {
        encoding: EncodingChoice::Manual("utf-16le".into()),
    };
    assert_eq!(
        DslReader.inspect(&bundle(&path), &options, &ctx)?.title,
        "Тестовый словарь"
    );
    Ok(())
}

#[test]
fn ошибки_кодировки_структуры_и_отмена_не_теряют_статьи() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("bad.dsl");
    let ctx = RunContext::new(temp.path().join("work"));
    std::fs::write(&path, b"word\n \xff\n")?;
    let options = ReadOptions {
        encoding: EncodingChoice::Manual("utf-8".into()),
    };
    assert_eq!(
        DslReader
            .read(&bundle(&path), &options, &mut CollectSink::default(), &ctx)
            .expect_err("Некорректный UTF-8")
            .code,
        "invalid_encoding"
    );
    std::fs::write(&path, "#NAME \"Empty\"\n")?;
    assert_eq!(
        DslReader
            .read(
                &bundle(&path),
                &ReadOptions::default(),
                &mut CollectSink::default(),
                &ctx
            )
            .expect_err("Пустой словарь")
            .code,
        "empty_dictionary"
    );
    std::fs::write(&path, " orphan body\n")?;
    let error = DslReader
        .read(
            &bundle(&path),
            &ReadOptions::default(),
            &mut CollectSink::default(),
            &ctx,
        )
        .expect_err("Тело без заголовка");
    assert_eq!(error.code, "invalid_dsl");
    assert_eq!(error.source_location.ok_or("Номер строки")?.line, 1);
    ctx.control.cancel();
    assert_eq!(
        DslReader
            .inspect(&bundle(&path), &ReadOptions::default(), &ctx)
            .expect_err("Отмена")
            .code,
        "cancelled"
    );
    Ok(())
}

struct Chunks<R> {
    inner: R,
    size: usize,
}
impl<R: Read> Read for Chunks<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let size = self.size.min(buffer.len());
        self.inner.read(&mut buffer[..size])
    }
}
