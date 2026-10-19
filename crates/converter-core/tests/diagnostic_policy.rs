use converter_core::{model::*, pipeline::convert_to_staging};
use serde_json::Value;
use std::sync::{Arc, Mutex};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn request(root: &std::path::Path, body: &str, abbreviations: Option<&str>) -> ConversionRequest {
    let source = root.join("source.dsl");
    std::fs::write(&source, body).expect("Исходный словарь");
    ConversionRequest {
        bundle: InputBundle {
            includes: vec![],
            main: PreparedFile {
                id: "main".into(),
                display_name: "source.dsl".into(),
                path: source,
            },
            abbreviations: abbreviations.map(|text| {
                let path = root.join("source_abrv.dsl");
                std::fs::write(&path, text).expect("Словарь сокращений");
                PreparedFile {
                    id: "abbr".into(),
                    display_name: "source_abrv.dsl".into(),
                    path,
                }
            }),
            resources: vec![],
        },
        options: ReadOptions::default(),
        metadata_override: None,
        staging_dir: root.join("ready"),
    }
}

fn context(root: &std::path::Path) -> (RunContext, Arc<Mutex<Vec<Value>>>) {
    let mut ctx = RunContext::new(root.join("scratch"));
    let events = Arc::new(Mutex::new(Vec::new()));
    let captured = events.clone();
    ctx.progress = Arc::new(move |event| {
        if let CoreEvent::Diagnostic(diagnostic) = event {
            captured
                .lock()
                .expect("Диагностика")
                .push(serde_json::to_value(diagnostic).expect("JSON"));
        }
    });
    (ctx, events)
}

fn report(artifacts: &ConvertedArtifacts) -> Value {
    serde_json::from_slice(&std::fs::read(&artifacts.report).expect("Отчёт")).expect("JSON отчёта")
}

#[test]
fn восстановление_оформления_остаётся_в_отчёте_без_предупреждения() -> TestResult {
    let temp = tempfile::tempdir()?;
    let request = request(
        temp.path(),
        "word\n [b]Жирный [i]курсив[/b] курсив[/i]\n",
        None,
    );
    let (ctx, events) = context(temp.path());
    let result = convert_to_staging(&request, &ctx)?;
    assert_eq!(result.summary.warnings, 0);
    assert!(events.lock().expect("Диагностика").is_empty());
    let report = report(&result);
    assert_eq!(report["diagnostics"][0]["code"], "repaired_markup");
    assert_eq!(report["diagnostics"][0]["severity"], "info");
    assert_eq!(report["diagnostic_counts"]["info_occurrences"], 1);
    Ok(())
}

#[test]
fn отсутствующий_файл_сокращений_даёт_одно_информационное_сообщение() -> TestResult {
    let temp = tempfile::tempdir()?;
    let request = request(
        temp.path(),
        "one\n [p]n.[/p] [p]v.[/p]\ntwo\n [p]n.[/p]\n",
        None,
    );
    let (ctx, events) = context(temp.path());
    let result = convert_to_staging(&request, &ctx)?;
    assert_eq!(result.summary.warnings, 0);
    assert!(events.lock().expect("Диагностика").is_empty());
    let report = report(&result);
    let raw = report["diagnostics"]
        .as_array()
        .expect("Исходные сообщения");
    assert_eq!(raw.len(), 1);
    assert_eq!(raw[0]["code"], "missing_abbreviations_file");
    assert_eq!(raw[0]["severity"], "info");
    assert_eq!(raw[0]["source_location"]["source"], "source.dsl");
    assert_eq!(raw[0]["source_location"]["line"], 1);
    Ok(())
}

#[test]
fn словарь_без_помет_не_требует_файл_сокращений() -> TestResult {
    let temp = tempfile::tempdir()?;
    let request = request(temp.path(), "one\n Значение\n", None);
    let (ctx, _) = context(temp.path());
    let result = convert_to_staging(&request, &ctx)?;
    assert!(
        report(&result)["diagnostics"]
            .as_array()
            .expect("Сообщения")
            .is_empty()
    );
    Ok(())
}

#[test]
fn пропущенные_пометы_группируются_с_сохранением_каждого_вхождения() -> TestResult {
    let temp = tempfile::tempdir()?;
    let request = request(
        temp.path(),
        "one\n [p]n.[/p] [p]n.[/p]\ntwo\n [p]v.[/p] [p]n.[/p]\n",
        Some("adj.\n прилагательное\n"),
    );
    let (ctx, events) = context(temp.path());
    let result = convert_to_staging(&request, &ctx)?;
    assert_eq!(result.summary.warnings, 2);
    let events = events.lock().expect("Диагностика");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["occurrences"], 3);
    assert_eq!(events[1]["occurrences"], 1);
    let report = report(&result);
    assert_eq!(
        report["grouped_diagnostics"],
        serde_json::to_value(&*events)?
    );
    let raw = report["diagnostics"].as_array().expect("Вхождения");
    assert_eq!(raw.len(), 4);
    assert_eq!(
        raw.iter()
            .map(|d| d["source_location"]["line"]
                .as_u64()
                .expect("Строка источника"))
            .collect::<Vec<_>>(),
        vec![1, 1, 3, 3]
    );
    assert_eq!(report["diagnostic_counts"]["warning_occurrences"], 4);
    assert_eq!(report["diagnostic_counts"]["warning_groups"], 2);
    Ok(())
}

#[test]
fn ресурсы_группируются_по_нормализованному_имени_а_ссылки_проверяются() -> TestResult {
    let temp = tempfile::tempdir()?;
    let request = request(
        temp.path(),
        "one\n [s]Media/A.PNG[/s]\ntwo\n [s]media\\\\a.png[/s] [s]media/b.png[/s] [ref]absent[/ref]\n",
        None,
    );
    let (ctx, events) = context(temp.path());
    let result = convert_to_staging(&request, &ctx)?;
    assert_eq!(result.summary.warnings, 3);
    let events = events.lock().expect("Диагностика");
    assert_eq!(events.len(), 3);
    assert_eq!(events[0]["code"], "missing_resource");
    assert_eq!(events[0]["occurrences"], 2);
    assert_eq!(events[1]["occurrences"], 1);
    assert_eq!(events[2]["code"], "missing_reference");
    let report = report(&result);
    assert_eq!(
        report["diagnostics"].as_array().expect("Вхождения").len(),
        4
    );
    assert_eq!(report["diagnostics"][0]["source_location"]["line"], 1);
    assert_eq!(report["diagnostics"][1]["source_location"]["line"], 3);
    Ok(())
}

#[test]
fn ошибка_после_диагностики_сохраняет_группы_без_дубликатов() -> TestResult {
    let temp = tempfile::tempdir()?;
    let request = request(
        temp.path(),
        "one\n [s]gone.png[/s] [s]gone.png[/s]\ntwo\n [s]../bad.png[/s]\n",
        None,
    );
    let (ctx, events) = context(temp.path());
    assert_eq!(
        convert_to_staging(&request, &ctx)
            .expect_err("Ошибка пути")
            .code,
        "unsafe_archive_path"
    );
    let events = events.lock().expect("Диагностика");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["occurrences"], 2);
    assert!(!request.staging_dir.exists());
    Ok(())
}

#[test]
fn отмена_после_чтения_сохраняет_накопленные_группы() -> TestResult {
    let temp = tempfile::tempdir()?;
    let request = request(
        temp.path(),
        "one\n [s]gone.png[/s] [s]gone.png[/s]\ntwo\n Статья\n",
        None,
    );
    let (mut ctx, events) = context(temp.path());
    let capture = ctx.progress.clone();
    let control = ctx.control.clone();
    ctx.progress = Arc::new(move |event| {
        if matches!(&event, CoreEvent::Progress(progress) if progress.stage == ConversionStage::Read && progress.articles == 1)
        {
            control.cancel();
        }
        capture(event);
    });
    assert_eq!(
        convert_to_staging(&request, &ctx).expect_err("Отмена").code,
        "cancelled"
    );
    let events = events.lock().expect("Диагностика");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["occurrences"], 2);
    assert!(!request.staging_dir.exists());
    Ok(())
}

#[test]
fn старые_диагностические_записи_читаются_как_одно_предупреждение() -> TestResult {
    let diagnostic: Diagnostic =
        serde_json::from_str(r#"{"code":"unknown_tag","message":"Тег","source_location":null}"#)?;
    let value = serde_json::to_value(diagnostic)?;
    assert_eq!(value["severity"], "warning");
    assert_eq!(value["occurrences"], 1);
    Ok(())
}
