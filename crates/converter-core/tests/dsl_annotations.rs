use converter_core::{
    dsl::DslReader, formats::DictionaryReader, model::*, pipeline::convert_to_staging,
    prepare::prepare_source,
};
use std::{fs::File, io::Write, path::Path};
use zip::{ZipWriter, write::SimpleFileOptions};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
const CREDIT: &str = "Конвертировано с помощью Converter — https://darkbearru.github.io/converter/";

fn source(path: &Path, id: &str) -> PreparedFile {
    PreparedFile {
        id: id.into(),
        display_name: path
            .file_name()
            .expect("Имя источника")
            .to_string_lossy()
            .into(),
        path: path.into(),
    }
}

fn utf16(text: &str) -> Vec<u8> {
    std::iter::once(0xfeff)
        .chain(text.encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect()
}

fn inspect(path: &Path, ctx: &RunContext) -> Result<DictionaryMetadata> {
    let bundle = prepare_source(&source(path, "main"), &[], ctx)?.remove(0);
    DslReader.inspect(&bundle, &ReadOptions::default(), ctx)
}

fn parts(root: &Path) -> TestResult {
    for number in 1..=3 {
        let include = if number == 1 {
            "#INCLUDE \"dict_2.dsl\"\n#INCLUDE \"dict_3.dsl\"\n"
        } else {
            ""
        };
        std::fs::write(
            root.join(format!("dict_{number}.dsl")),
            utf16(&format!(
                "#NAME \"大БКРС - 260610 ( {number} / 3 )\"\n{include}entry{number}\n body\n"
            )),
        )?;
    }
    Ok(())
}

#[test]
fn utf16_аннотация_главной_части_даёт_описание_и_общее_название() -> TestResult {
    let temp = tempfile::tempdir()?;
    parts(temp.path())?;
    for number in 1..=3 {
        std::fs::write(
            temp.path().join(format!("dict_{number}.ann")),
            utf16(&format!(
                "Автор: Иван.\r\nИсточник: https://bkrs.info\r\nЛицензия: CC BY\r\nКонвертировано с помощью https://github.com/dictz\r\nЧасть {number} из 3\r\n"
            )),
        )?;
    }
    let ctx = RunContext::new(temp.path().join("work"));
    let bundle =
        prepare_source(&source(&temp.path().join("dict_1.dsl"), "main"), &[], &ctx)?.remove(0);
    let metadata = DslReader.inspect(&bundle, &ReadOptions::default(), &ctx)?;
    assert_eq!(metadata.title, "大БКРС - 260610");
    assert_eq!(
        metadata.description,
        format!("Автор: Иван.\nИсточник: https://bkrs.info\nЛицензия: CC BY\n{CREDIT}")
    );
    assert!(
        bundle.resources.is_empty(),
        "Аннотации являются метаданными"
    );
    Ok(())
}

#[test]
fn аннотация_главной_части_имеет_приоритет_перед_включённой() -> TestResult {
    let temp = tempfile::tempdir()?;
    parts(temp.path())?;
    std::fs::write(temp.path().join("dict_1.ann"), "Главное описание")?;
    std::fs::write(temp.path().join("dict_2.ann"), "Описание другой части")?;
    let ctx = RunContext::new(temp.path().join("work"));
    assert_eq!(
        inspect(&temp.path().join("dict_1.dsl"), &ctx)?.description,
        format!("Главное описание\n{CREDIT}")
    );
    Ok(())
}

#[test]
fn без_главной_аннотации_используется_аннотация_включённой_части() -> TestResult {
    let temp = tempfile::tempdir()?;
    parts(temp.path())?;
    std::fs::write(
        temp.path().join("dict_3.ann"),
        utf16("Описание словаря\nЧасть 3 из 3\n"),
    )?;
    let ctx = RunContext::new(temp.path().join("work"));
    assert_eq!(
        inspect(&temp.path().join("dict_1.dsl"), &ctx)?.description,
        format!("Описание словаря\n{CREDIT}")
    );
    Ok(())
}

#[test]
fn отдельная_часть_сохраняет_номер_в_названии_и_аннотации() -> TestResult {
    let temp = tempfile::tempdir()?;
    parts(temp.path())?;
    std::fs::write(temp.path().join("dict_2.ann"), "Часть 2 из 3\nАвтор: тест")?;
    let ctx = RunContext::new(temp.path().join("work"));
    let metadata = inspect(&temp.path().join("dict_2.dsl"), &ctx)?;
    assert_eq!(metadata.title, "大БКРС - 260610 ( 2 / 3 )");
    assert_eq!(
        metadata.description,
        format!("Часть 2 из 3\nАвтор: тест\n{CREDIT}")
    );
    Ok(())
}

#[test]
fn включение_другой_редакции_не_удаляет_номер_части() -> TestResult {
    let temp = tempfile::tempdir()?;
    parts(temp.path())?;
    std::fs::write(
        temp.path().join("dict_3.dsl"),
        "#NAME \"大БКРС - 260611 ( 3 / 3 )\"\nentry3\n body\n",
    )?;
    let ctx = RunContext::new(temp.path().join("work"));
    assert_eq!(
        inspect(&temp.path().join("dict_1.dsl"), &ctx)?.title,
        "大БКРС - 260610 ( 1 / 3 )"
    );
    Ok(())
}

#[test]
fn ann_находится_рядом_со_сжатым_dsl() -> TestResult {
    use flate2::{Compression, write::GzEncoder};
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("dict.dsl.dz");
    let mut gzip = GzEncoder::new(File::create(&path)?, Compression::default());
    gzip.write_all(b"#NAME \"Dictionary\"\nentry\n body\n")?;
    gzip.finish()?;
    std::fs::write(
        temp.path().join("dict.ann"),
        utf16("Описание сжатого словаря"),
    )?;
    let ctx = RunContext::new(temp.path().join("work"));
    assert_eq!(
        inspect(&path, &ctx)?.description,
        format!("Описание сжатого словаря\n{CREDIT}")
    );
    Ok(())
}

#[test]
fn zip_подбирает_аннотацию_в_той_же_папке() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("dict.zip");
    let mut zip = ZipWriter::new(File::create(&path)?);
    for (name, bytes) in [
        (
            "dict/main.dsl",
            b"#NAME \"Dictionary\"\nentry\n body\n".to_vec(),
        ),
        ("dict/main.ann", utf16("Описание из ZIP")),
        ("other/main.ann", b"Wrong annotation".to_vec()),
    ] {
        zip.start_file(name, SimpleFileOptions::default())?;
        zip.write_all(&bytes)?;
    }
    zip.finish()?;
    let ctx = RunContext::new(temp.path().join("work"));
    assert_eq!(
        inspect(&path, &ctx)?.description,
        format!("Описание из ZIP\n{CREDIT}")
    );
    Ok(())
}

#[test]
fn аннотация_из_другого_zip_не_подменяет_отсутствующую() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("dict.zip");
    let other = temp.path().join("other.zip");
    for (path, name, bytes) in [
        (&path, "main.dsl", b"entry\n body\n".as_slice()),
        (&other, "main.ann", b"Wrong annotation".as_slice()),
    ] {
        let mut zip = ZipWriter::new(File::create(path)?);
        zip.start_file(name, SimpleFileOptions::default())?;
        zip.write_all(bytes)?;
        zip.finish()?;
    }
    let ctx = RunContext::new(temp.path().join("work"));
    let bundle =
        prepare_source(&source(&path, "main"), &[source(&other, "other")], &ctx)?.remove(0);
    assert_eq!(
        DslReader
            .inspect(&bundle, &ReadOptions::default(), &ctx)?
            .description,
        CREDIT
    );
    Ok(())
}

#[test]
fn авторство_лицензия_и_источник_сохраняются_при_замене_английского_кредита() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("dict.dsl");
    std::fs::write(&path, "entry\n body\n")?;
    std::fs::write(
        temp.path().join("dict.ann"),
        "Author: John\nLicense: CC BY\nSource: https://example.org\nConverted by OldTool\n",
    )?;
    let ctx = RunContext::new(temp.path().join("work"));
    assert_eq!(
        inspect(&path, &ctx)?.description,
        "Author: John\nLicense: CC BY\nSource: https://example.org\nConverted with Converter — https://darkbearru.github.io/converter/"
    );
    Ok(())
}

#[test]
fn пустое_описание_переопределения_не_стирает_ann_а_непустое_заменяет() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("dict.dsl");
    std::fs::write(&path, "entry\n body\n")?;
    std::fs::write(temp.path().join("dict.ann"), "Описание из ANN")?;
    let ctx = RunContext::new(temp.path().join("work"));
    let bundle = prepare_source(&source(&path, "main"), &[], &ctx)?.remove(0);
    for (number, description, expected) in [
        (0, "", format!("Описание из ANN\n{CREDIT}")),
        (1, "Моё описание", "Моё описание".into()),
    ] {
        let result = convert_to_staging(
            &ConversionRequest {
                bundle: bundle.clone(),
                options: ReadOptions::default(),
                metadata_override: Some(DictionaryMetadata {
                    title: "Мой заголовок".into(),
                    description: description.into(),
                    ..Default::default()
                }),
                staging_dir: temp.path().join(format!("ready-{number}")),
            },
            &ctx,
        )?;
        let report: serde_json::Value = serde_json::from_reader(File::open(result.report)?)?;
        assert_eq!(report["metadata"]["title"], "Мой заголовок");
        assert_eq!(report["metadata"]["description"], expected);
        let dictionary = mdictlib::MdxFile::open(result.mdx)?;
        assert_eq!(dictionary.header().title(), Some("Мой заголовок"));
        assert_eq!(dictionary.header().description(), Some(expected.as_str()));
    }
    Ok(())
}

#[test]
fn аннотация_ограничена_общим_размером_а_не_только_длиной_строки() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("dict.dsl");
    std::fs::write(&path, "entry\n body\n")?;
    std::fs::write(temp.path().join("dict.ann"), "abcd\n".repeat(100))?;
    let mut ctx = RunContext::new(temp.path().join("work"));
    ctx.limits.max_article_bytes = 64;
    assert_eq!(
        inspect(&path, &ctx).expect_err("Лимит аннотации").code,
        "article_too_large"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn ann_не_читается_через_символическую_ссылку() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("dict.dsl");
    std::fs::write(&path, "entry\n body\n")?;
    let outside = tempfile::tempdir()?;
    std::fs::write(outside.path().join("secret.ann"), "Запрещено")?;
    std::os::unix::fs::symlink(
        outside.path().join("secret.ann"),
        temp.path().join("dict.ann"),
    )?;
    let ctx = RunContext::new(temp.path().join("work"));
    assert_eq!(
        prepare_source(&source(&path, "main"), &[], &ctx)
            .expect_err("Ссылка")
            .code,
        "unsafe_archive_path"
    );
    Ok(())
}

#[test]
fn аннотация_ровно_на_границе_лимита_допустима() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("dict.dsl");
    std::fs::write(&path, "entry\n body\n")?;
    std::fs::write(temp.path().join("dict.ann"), "a".repeat(64))?;
    let mut ctx = RunContext::new(temp.path().join("work"));
    ctx.limits.max_article_bytes = 64;
    assert!(
        inspect(&path, &ctx)?
            .description
            .starts_with(&"a".repeat(64))
    );
    Ok(())
}

#[test]
fn языковые_секции_сохраняются_с_подписями_и_кредитом_каждого_языка() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("dict.dsl");
    std::fs::write(&path, "entry\n body\n")?;
    std::fs::write(
        temp.path().join("dict.ann"),
        "Общие сведения\n#LANGUAGE \"Russian\"\nАвтор: Иван\nЛицензия: CC BY\n#LANGUAGE \"English\"\nAuthor: John\nLicense: CC BY\n#LANGUAGE \"Klingon\"\nOriginal author: Qapla\n",
    )?;
    let ctx = RunContext::new(temp.path().join("work"));
    assert_eq!(
        inspect(&path, &ctx)?.description,
        format!(
            "Общие сведения\n\nRussian:\nАвтор: Иван\nЛицензия: CC BY\n{CREDIT}\n\nEnglish:\nAuthor: John\nLicense: CC BY\nConverted with Converter — https://darkbearru.github.io/converter/\n\nKlingon:\nOriginal author: Qapla\nConverted with Converter — https://darkbearru.github.io/converter/"
        )
    );
    Ok(())
}

#[test]
fn языковые_кредиты_заменяются_без_дублей_а_другие_строки_сохраняются() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("dict.dsl");
    std::fs::write(&path, "entry\n body\n")?;
    std::fs::write(
        temp.path().join("dict.ann"),
        "#LANGUAGE \"Russian\"\nАвтор: Иван\nConverted by OldTool\nКонвертировано с помощью Converter — https://darkbearru.github.io/converter/\n#LANGUAGE \"English\"\nCopyright: Original Publisher\nЛицензия: CC BY\nКонвертация: OldTool\nConverted with Converter — https://darkbearru.github.io/converter/\n",
    )?;
    let ctx = RunContext::new(temp.path().join("work"));
    assert_eq!(
        inspect(&path, &ctx)?.description,
        format!(
            "Russian:\nАвтор: Иван\n{CREDIT}\n\nEnglish:\nCopyright: Original Publisher\nЛицензия: CC BY\nConverted with Converter — https://darkbearru.github.io/converter/"
        )
    );
    Ok(())
}

#[test]
fn ann_без_bom_использует_реальную_ansi_кодировку_словаря() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("dict.dsl");
    let text = "#SOURCE_CODE_PAGE \"Latin\"\n#NAME \"café\"\nentry\n body\n";
    std::fs::write(&path, encoding_rs::WINDOWS_1252.encode(text).0)?;
    std::fs::write(
        temp.path().join("dict.ann"),
        encoding_rs::WINDOWS_1252
            .encode("#LANGUAGE \"English\"\nAuthor: André\n")
            .0,
    )?;
    let ctx = RunContext::new(temp.path().join("work"));
    assert_eq!(
        inspect(&path, &ctx)?.description,
        "English:\nAuthor: André\nConverted with Converter — https://darkbearru.github.io/converter/"
    );
    Ok(())
}

#[test]
fn ann_utf8_без_bom_не_переопределяется_ansi_кодировкой_словаря() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("dict.dsl");
    std::fs::write(
        &path,
        encoding_rs::WINDOWS_1251
            .encode("#SOURCE_CODE_PAGE \"Cyrillic\"\nслово\n текст\n")
            .0,
    )?;
    std::fs::write(
        temp.path().join("dict.ann"),
        "#LANGUAGE \"English\"\nAuthor: André\n",
    )?;
    let ctx = RunContext::new(temp.path().join("work"));
    assert_eq!(
        inspect(&path, &ctx)?.description,
        "English:\nAuthor: André\nConverted with Converter — https://darkbearru.github.io/converter/"
    );
    Ok(())
}

#[test]
fn ann_включённой_части_наследует_кодировку_этой_части() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("main.dsl");
    std::fs::write(
        &path,
        "#NAME \"Dictionary (1/2)\"\n#INCLUDE \"part.dsl\"\nentry\n body\n",
    )?;
    std::fs::write(
        temp.path().join("part.dsl"),
        encoding_rs::WINDOWS_1252
            .encode("#SOURCE_CODE_PAGE \"Latin\"\n#NAME \"Dictionary (2/2)\"\ncafé\n body\n")
            .0,
    )?;
    std::fs::write(
        temp.path().join("part.ann"),
        encoding_rs::WINDOWS_1252
            .encode("#LANGUAGE \"English\"\nAuthor: André\n")
            .0,
    )?;
    let ctx = RunContext::new(temp.path().join("work"));
    assert_eq!(
        inspect(&path, &ctx)?.description,
        "English:\nAuthor: André\nConverted with Converter — https://darkbearru.github.io/converter/"
    );
    Ok(())
}

#[test]
fn номер_объединённой_части_очищается_во_всех_языках_ann() -> TestResult {
    let temp = tempfile::tempdir()?;
    parts(temp.path())?;
    std::fs::write(
        temp.path().join("dict_1.ann"),
        "#LANGUAGE \"Russian\"\nАвтор: Иван\nЧасть 1 из 3\n#LANGUAGE \"English\"\nAuthor: John\nPart 1 of 3\n",
    )?;
    let ctx = RunContext::new(temp.path().join("work"));
    assert_eq!(
        inspect(&temp.path().join("dict_1.dsl"), &ctx)?.description,
        format!(
            "Russian:\nАвтор: Иван\n{CREDIT}\n\nEnglish:\nAuthor: John\nConverted with Converter — https://darkbearru.github.io/converter/"
        )
    );
    Ok(())
}
