mod support;
use converter_core::{
    dsl::DslReader, formats::DictionaryReader, model::*, prepare::prepare_source,
};
use std::{fs::File, io::Write};
use support::collect_sink::CollectSink;
use zip::{ZipWriter, write::SimpleFileOptions};
type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
fn source(path: std::path::PathBuf) -> PreparedFile {
    PreparedFile {
        id: "main".into(),
        display_name: path
            .file_name()
            .expect("source filename")
            .to_string_lossy()
            .into(),
        path,
    }
}
#[test]
fn include_объединяет_соседние_части_и_сохраняет_главные_метаданные() -> TestResult {
    let temp = tempfile::tempdir()?;
    let main = temp.path().join("main.dsl");
    std::fs::write(
        &main,
        "#NAME \"Main\"\n#INCLUDE \"part.dsl\"\nroot\n body\n",
    )?;
    std::fs::write(
        temp.path().join("part.dsl"),
        "#NAME \"Part\"\n#INCLUDE \"last.dsl\"\npart\n body\n",
    )?;
    std::fs::write(temp.path().join("last.dsl"), "last\n body\n")?;
    let ctx = RunContext::new(temp.path().join("work"));
    let bundles = prepare_source(&source(main), &[], &ctx)?;
    assert_eq!(bundles.len(), 1);
    assert_eq!(
        DslReader
            .inspect(&bundles[0], &ReadOptions::default(), &ctx)?
            .title,
        "Main"
    );
    let mut sink = CollectSink::default();
    let summary = DslReader.read(&bundles[0], &ReadOptions::default(), &mut sink, &ctx)?;
    assert_eq!(summary.source_articles, 3);
    assert_eq!(
        sink.entries.iter().map(|e| e.id).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert!(
        !sink
            .diagnostics
            .iter()
            .any(|d| d.code == "unknown_directive")
    );
    Ok(())
}
#[test]
fn include_в_zip_не_предлагает_части_как_отдельные_словари() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("dictionary.zip");
    let mut zip = ZipWriter::new(File::create(&path)?);
    for (name, text) in [
        ("dict/main.dsl", "#INCLUDE \"part.dsl\"\nroot\n body\n"),
        ("dict/part.dsl", "part\n body\n"),
    ] {
        zip.start_file(name, SimpleFileOptions::default())?;
        zip.write_all(text.as_bytes())?;
    }
    zip.finish()?;
    let ctx = RunContext::new(temp.path().join("work"));
    let bundles = prepare_source(&source(path), &[], &ctx)?;
    assert_eq!(bundles.len(), 1);
    let mut sink = CollectSink::default();
    assert_eq!(
        DslReader
            .read(&bundles[0], &ReadOptions::default(), &mut sink, &ctx)?
            .source_articles,
        2
    );
    Ok(())
}
#[test]
fn include_сообщает_об_отсутствии_цикле_и_выходе_из_папки() -> TestResult {
    let temp = tempfile::tempdir()?;
    let main = temp.path().join("main.dsl");
    let ctx = RunContext::new(temp.path().join("work"));
    for (target, code) in [
        ("missing.dsl", "include_not_found"),
        ("main.dsl", "include_cycle"),
        ("../outside.dsl", "unsafe_include_path"),
    ] {
        std::fs::write(&main, format!("#INCLUDE \"{target}\"\nroot\n body\n"))?;
        let error = prepare_source(&source(main.clone()), &[], &ctx).expect_err("invalid include");
        assert_eq!(error.code, code);
        assert_eq!(error.source_location.as_ref().map(|l| l.line), Some(1));
    }
    Ok(())
}

#[test]
fn ссылка_из_включённой_части_ссылается_на_главную_а_ошибка_сохраняет_имя_части() -> TestResult {
    use converter_core::pipeline::convert_to_staging;
    let temp = tempfile::tempdir()?;
    let main = temp.path().join("main.dsl");
    std::fs::write(&main, "#INCLUDE \"part.dsl\"\nroot\n body\n")?;
    std::fs::write(
        temp.path().join("part.dsl"),
        "part\n [ref]root[/ref] [ref]missing[/ref]\n",
    )?;
    let ctx = RunContext::new(temp.path().join("work"));
    let bundle = prepare_source(&source(main), &[], &ctx)?.remove(0);
    let result = convert_to_staging(
        &ConversionRequest {
            bundle,
            options: ReadOptions::default(),
            metadata_override: None,
            staging_dir: temp.path().join("ready"),
        },
        &ctx,
    )?;
    let report: serde_json::Value = serde_json::from_slice(&std::fs::read(result.report)?)?;
    let warnings: Vec<_> = report["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .filter(|d| d["code"] == "missing_reference")
        .collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0]["source_location"]["source"], "part.dsl");
    assert_eq!(warnings[0]["source_location"]["line"], 1);
    Ok(())
}

#[test]
fn ручная_кодировка_применяется_и_к_включённым_частям_без_bom() -> TestResult {
    use converter_core::prepare::prepare_source_with_options;
    let temp = tempfile::tempdir()?;
    let main = temp.path().join("main.dsl");
    for (file, text) in [
        (&main, "#INCLUDE \"part.dsl\"\nroot\n body\n"),
        (&temp.path().join("part.dsl"), "part\n body\n"),
    ] {
        std::fs::write(
            file,
            text.encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        )?;
    }
    let ctx = RunContext::new(temp.path().join("work"));
    let options = ReadOptions {
        encoding: EncodingChoice::Manual("utf-16le".into()),
    };
    let bundle = prepare_source_with_options(&source(main), &[], &ctx, &options)?.remove(0);
    let mut sink = CollectSink::default();
    assert_eq!(
        DslReader
            .read(&bundle, &options, &mut sink, &ctx)?
            .source_articles,
        2
    );
    Ok(())
}

#[test]
fn вложенное_включение_не_зависит_от_расширения_части() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("dictionary.zip");
    let mut zip = ZipWriter::new(File::create(&path)?);
    for (name, text) in [
        ("part.inc", "#INCLUDE \"last.dsl\"\npart\n body\n"),
        ("main.dsl", "#INCLUDE \"part.inc\"\nroot\n body\n"),
        ("last.dsl", "last\n body\n"),
    ] {
        zip.start_file(name, SimpleFileOptions::default())?;
        zip.write_all(text.as_bytes())?;
    }
    zip.finish()?;
    let ctx = RunContext::new(temp.path().join("work"));
    let bundles = prepare_source(&source(path), &[], &ctx)?;
    assert_eq!(bundles.len(), 1);
    let mut sink = CollectSink::default();
    assert_eq!(
        DslReader
            .read(&bundles[0], &ReadOptions::default(), &mut sink, &ctx)?
            .source_articles,
        3
    );
    assert!(bundles[0].resources.is_empty());
    Ok(())
}

#[test]
fn повторно_выбранная_та_же_часть_не_создаёт_неоднозначность() -> TestResult {
    let temp = tempfile::tempdir()?;
    let main = temp.path().join("main.dsl");
    let part = temp.path().join("part.dsl");
    std::fs::write(&main, "#INCLUDE \"part.dsl\"\nroot\n body\n")?;
    std::fs::write(&part, "part\n body\n")?;
    let attachments = [
        PreparedFile {
            id: "part".into(),
            display_name: "part.dsl".into(),
            path: part,
        },
        PreparedFile {
            id: "folder".into(),
            display_name: "folder".into(),
            path: temp.path().into(),
        },
    ];
    let ctx = RunContext::new(temp.path().join("work"));
    let bundle = prepare_source(&source(main), &attachments, &ctx)?.remove(0);
    let mut sink = CollectSink::default();
    assert_eq!(
        DslReader
            .read(&bundle, &ReadOptions::default(), &mut sink, &ctx)?
            .source_articles,
        2
    );
    Ok(())
}

#[test]
fn include_не_подменяется_одноимённым_приложенным_файлом() -> TestResult {
    let temp = tempfile::tempdir()?;
    std::fs::create_dir(temp.path().join("other"))?;
    let main = temp.path().join("main.dsl");
    let wrong = temp.path().join("other/part.dsl");
    std::fs::write(&main, "#INCLUDE \"part.dsl\"\nroot\n body\n")?;
    std::fs::write(temp.path().join("part.dsl"), "correct-entry\n body\n")?;
    std::fs::write(&wrong, "wrong-entry\n body\n")?;
    let attachment = PreparedFile {
        id: "other".into(),
        display_name: "part.dsl".into(),
        path: wrong,
    };
    let ctx = RunContext::new(temp.path().join("work"));
    let bundle = prepare_source(&source(main), &[attachment], &ctx)?.remove(0);
    let mut sink = CollectSink::default();
    DslReader.read(&bundle, &ReadOptions::default(), &mut sink, &ctx)?;
    assert!(
        sink.entries
            .iter()
            .any(|e| e.headwords == ["correct-entry"])
    );
    assert!(!sink.entries.iter().any(|e| e.headwords == ["wrong-entry"]));
    Ok(())
}

#[test]
fn include_не_ищет_часть_в_другом_zip() -> TestResult {
    let temp = tempfile::tempdir()?;
    let main = temp.path().join("main.zip");
    let other = temp.path().join("other.zip");
    for (path, name, text) in [
        (&main, "main.dsl", "#INCLUDE \"part.dsl\"\nroot\n body\n"),
        (&other, "part.dsl", "wrong\n body\n"),
    ] {
        let mut zip = ZipWriter::new(File::create(path)?);
        zip.start_file(name, SimpleFileOptions::default())?;
        zip.write_all(text.as_bytes())?;
        zip.finish()?;
    }
    let ctx = RunContext::new(temp.path().join("work"));
    let attachment = PreparedFile {
        id: "other".into(),
        display_name: "other.zip".into(),
        path: other,
    };
    assert_eq!(
        prepare_source(&source(main), &[attachment], &ctx)
            .expect_err("missing part in own archive")
            .code,
        "include_not_found"
    );
    Ok(())
}

#[test]
fn include_использует_собственную_кодировку_каждой_части() -> TestResult {
    use converter_core::prepare::prepare_source_with_options;
    let temp = tempfile::tempdir()?;
    let main = temp.path().join("main.dsl");
    std::fs::write(
        &main,
        "#INCLUDE \"part.dsl\"\nroot\n body\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    )?;
    std::fs::write(
        temp.path().join("part.dsl"),
        "\u{feff}#INCLUDE \"last.dsl\"\npart\n body\n",
    )?;
    std::fs::write(temp.path().join("last.dsl"), "last\n body\n")?;
    let ctx = RunContext::new(temp.path().join("work"));
    let options = ReadOptions {
        encoding: EncodingChoice::Manual("utf-16le".into()),
    };
    let bundle = prepare_source_with_options(&source(main), &[], &ctx, &options)?.remove(0);
    let mut sink = CollectSink::default();
    assert_eq!(
        DslReader
            .read(&bundle, &options, &mut sink, &ctx)?
            .source_articles,
        3
    );
    Ok(())
}
