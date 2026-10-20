mod support;

use converter_core::{
    dsl::{DslReader, decoder::resolve_encoding},
    formats::DictionaryReader,
    model::*,
};
use encoding_rs::{
    Encoding, UTF_8, WINDOWS_874, WINDOWS_1250, WINDOWS_1251, WINDOWS_1252, WINDOWS_1253,
    WINDOWS_1254, WINDOWS_1255, WINDOWS_1256, WINDOWS_1257, WINDOWS_1258,
};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use support::collect_sink::CollectSink;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn source(path: &Path) -> PreparedFile {
    PreparedFile {
        id: path
            .file_name()
            .expect("Имя файла")
            .to_string_lossy()
            .into(),
        display_name: path
            .file_name()
            .expect("Имя файла")
            .to_string_lossy()
            .into(),
        path: path.into(),
    }
}

fn bundle(path: &Path) -> InputBundle {
    InputBundle {
        main: source(path),
        includes: vec![],
        annotation: None,
        abbreviations: None,
        resources: vec![],
    }
}

fn write_encoded(path: &Path, text: &str, encoding: &'static Encoding) -> TestResult {
    let (bytes, _, malformed) = encoding.encode(text);
    assert!(
        !malformed,
        "Текст фикстуры должен представляться в кодировке"
    );
    std::fs::write(path, bytes)?;
    Ok(())
}

#[test]
fn директива_выбирает_кодировку_заголовков_и_статей_без_предупреждения() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("dict.dsl");
    let ctx = RunContext::new(temp.path().join("work"));
    for (label, encoding, word) in [
        ("EasternEuropean", WINDOWS_1250, "Łódź"),
        ("Cyrillic", WINDOWS_1251, "слово"),
        ("Latin", WINDOWS_1252, "café"),
        ("Greek", WINDOWS_1253, "λέξη"),
        ("Turkish", WINDOWS_1254, "sözlük"),
        ("Hebrew", WINDOWS_1255, "שלום"),
        ("Arabic", WINDOWS_1256, "كتاب"),
        ("Baltic", WINDOWS_1257, "žodis"),
        ("Vietnamese", WINDOWS_1258, "xin chào"),
        ("Thai", WINDOWS_874, "ภาษา"),
    ] {
        write_encoded(
            &path,
            &format!("#SOURCE_CODE_PAGE \"{label}\"\n#NAME \"{word}\"\n{word}\n {word}\n"),
            encoding,
        )?;
        let input = bundle(&path);
        let resolved = resolve_encoding(&input.main, EncodingChoice::Auto, &ctx)?;
        assert_eq!(resolved.encoding, encoding, "{label}");
        assert!(!resolved.needs_confirmation, "{label}");
        let metadata = DslReader.inspect(&input, &ReadOptions::default(), &ctx)?;
        assert_eq!(metadata.title, word, "{label}");
        let mut sink = CollectSink::default();
        DslReader.read(&input, &ReadOptions::default(), &mut sink, &ctx)?;
        assert_eq!(sink.entries[0].headwords, [word], "{label}");
        assert!(sink.entries[0].html.contains(word), "{label}");
        assert!(
            sink.diagnostics.is_empty(),
            "{label}: {:?}",
            sink.diagnostics
        );
    }
    Ok(())
}

#[test]
fn все_поддержанные_windows_кодировки_доступны_для_ручного_выбора() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("dict.dsl");
    std::fs::write(&path, "entry\n body\n")?;
    let ctx = RunContext::new(temp.path().join("work"));
    for (label, encoding) in [
        ("windows-1250", WINDOWS_1250),
        ("windows-1251", WINDOWS_1251),
        ("windows-1252", WINDOWS_1252),
        ("windows-1253", WINDOWS_1253),
        ("windows-1254", WINDOWS_1254),
        ("windows-1255", WINDOWS_1255),
        ("windows-1256", WINDOWS_1256),
        ("windows-1257", WINDOWS_1257),
        ("windows-1258", WINDOWS_1258),
        ("windows-874", WINDOWS_874),
    ] {
        assert_eq!(
            resolve_encoding(&source(&path), EncodingChoice::Manual(label.into()), &ctx)?.encoding,
            encoding
        );
    }
    Ok(())
}

#[test]
fn неизвестная_объявленная_кодировка_даёт_локализуемую_ошибку() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("dict.dsl");
    std::fs::write(&path, "#SOURCE_CODE_PAGE \"Invented\"\nentry\n body\n")?;
    let ctx = RunContext::new(temp.path().join("work"));
    let error = resolve_encoding(&source(&path), EncodingChoice::Auto, &ctx)
        .expect_err("Неизвестная кодировка");
    assert_eq!(error.code, "unsupported_encoding");
    assert_eq!(
        error.message_key.as_deref(),
        Some("messages.unsupported_encoding")
    );
    assert_eq!(error.message_args["label"], "Invented");
    Ok(())
}

#[test]
fn ручной_выбор_и_unicode_bom_важнее_ansi_директивы() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("dict.dsl");
    let ctx = RunContext::new(temp.path().join("work"));
    let text = "#SOURCE_CODE_PAGE \"Invented\"\n#NAME \"Тест\"\nentry\n body\n";
    std::fs::write(&path, text)?;
    assert_eq!(
        resolve_encoding(&source(&path), EncodingChoice::Manual("utf-8".into()), &ctx)?.encoding,
        UTF_8
    );
    for (encoding, bytes) in [
        (
            UTF_8,
            [vec![0xef, 0xbb, 0xbf], text.as_bytes().to_vec()].concat(),
        ),
        (
            encoding_rs::UTF_16LE,
            std::iter::once(0xfeff)
                .chain(text.encode_utf16())
                .flat_map(u16::to_le_bytes)
                .collect(),
        ),
        (
            encoding_rs::UTF_16BE,
            std::iter::once(0xfeff)
                .chain(text.encode_utf16())
                .flat_map(u16::to_be_bytes)
                .collect(),
        ),
    ] {
        std::fs::write(&path, bytes)?;
        assert_eq!(
            resolve_encoding(&source(&path), EncodingChoice::Auto, &ctx)?.encoding,
            encoding
        );
    }
    Ok(())
}

#[test]
fn директива_в_комментарии_или_теле_не_выбирает_кодировку() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("dict.dsl");
    let ctx = RunContext::new(temp.path().join("work"));
    for text in [
        "{{\n#SOURCE_CODE_PAGE \"Invented\"\n}}\n#SOURCE_CODE_PAGE \"Latin\"\ncafé\n body\n",
        "#NAME \"Test\"\nentry\n #SOURCE_CODE_PAGE \"Invented\"\n café\n",
        "entry\n body\n#SOURCE_CODE_PAGE \"Invented\"\n café\n",
    ] {
        write_encoded(&path, text, WINDOWS_1252)?;
        let encoding = resolve_encoding(&source(&path), EncodingChoice::Auto, &ctx)?;
        assert_eq!(
            encoding.encoding,
            if text.starts_with("{{") {
                WINDOWS_1252
            } else {
                WINDOWS_1251
            }
        );
    }
    Ok(())
}

#[test]
fn объявленная_кодировка_не_требует_прохода_по_всему_словарю() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("dict.dsl");
    std::fs::write(
        &path,
        format!(
            "#SOURCE_CODE_PAGE \"Latin\"\nentry\n {}\n",
            "a".repeat(128 * 1024)
        ),
    )?;
    let events = Arc::new(Mutex::new(Vec::new()));
    let captured = events.clone();
    let mut ctx = RunContext::new(temp.path().join("work"));
    ctx.progress = Arc::new(move |event| captured.lock().expect("События").push(event));
    assert_eq!(
        resolve_encoding(&source(&path), EncodingChoice::Auto, &ctx)?.encoding,
        WINDOWS_1252
    );
    assert!(
        events.lock().expect("События").is_empty(),
        "После декларации не нужен проход Inspect по телу"
    );
    Ok(())
}

#[test]
fn включённый_словарь_и_сокращения_используют_свои_ansi_директивы() -> TestResult {
    let temp = tempfile::tempdir()?;
    let main = temp.path().join("dict.dsl");
    let included = temp.path().join("part.dsl");
    let abbreviations = temp.path().join("dict_abrv.dsl");
    std::fs::write(&main, "entry\n [p]n.[/p]\n")?;
    write_encoded(
        &included,
        "#SOURCE_CODE_PAGE \"Greek\"\nλέξη\n κείμενο\n",
        WINDOWS_1253,
    )?;
    write_encoded(
        &abbreviations,
        "#SOURCE_CODE_PAGE \"Latin\"\nn.\n café\n",
        WINDOWS_1252,
    )?;
    let mut input = bundle(&main);
    input.includes.push(source(&included));
    input.abbreviations = Some(source(&abbreviations));
    let mut sink = CollectSink::default();
    DslReader.read(
        &input,
        &ReadOptions::default(),
        &mut sink,
        &RunContext::new(temp.path().join("work")),
    )?;
    let main = sink
        .entries
        .iter()
        .find(|entry| entry.headwords == ["entry"])
        .expect("Основная статья");
    let included = sink
        .entries
        .iter()
        .find(|entry| entry.headwords == ["λέξη"])
        .expect("Включённая статья");
    assert!(main.html.contains("title=\"café\""));
    assert!(included.html.contains("κείμενο"));
    assert!(sink.diagnostics.is_empty());
    Ok(())
}
