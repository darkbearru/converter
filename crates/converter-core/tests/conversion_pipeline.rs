use converter_core::{
    dsl::renderer::CONVERTER_CSS,
    model::*,
    pipeline::{convert_to_staging, validate_artifacts},
};
use mdictlib::{ChecksumPolicy, MddFile, MdxFile, OpenOptions};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Write,
    sync::{Arc, Mutex},
};

#[test]
fn ручной_utf16_без_bom_проходит_полную_конвертацию()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    for (label, big_endian) in [("utf-16le", false), ("utf-16be", true)] {
        let temp = tempfile::tempdir()?;
        let mut request = fixture(temp.path(), 1)?;
        let text = "#NAME \"Проверка\"\nalpha\n\tСтатья\n";
        let bytes: Vec<u8> = text
            .encode_utf16()
            .flat_map(|c| {
                if big_endian {
                    c.to_be_bytes()
                } else {
                    c.to_le_bytes()
                }
            })
            .collect();
        std::fs::write(&request.bundle.main.path, bytes)?;
        let ctx = RunContext::new(temp.path().join("scratch"));
        assert_eq!(
            convert_to_staging(&request, &ctx)
                .expect_err("Нужен ручной выбор")
                .code,
            "encoding_required"
        );
        request.options.encoding = EncodingChoice::Manual(label.into());
        let result = convert_to_staging(&request, &ctx)?;
        assert_eq!(result.summary.source_articles, 1);
        let reader = MdxFile::open(&result.mdx)?;
        assert!(
            reader
                .lookup("alpha")?
                .ok_or("Статья")?
                .text()
                .contains("Статья")
        );
    }
    Ok(())
}

fn fixture(
    root: &std::path::Path,
    articles: u64,
) -> std::result::Result<ConversionRequest, Box<dyn std::error::Error>> {
    let source = root.join("source.dsl");
    let mut file = File::create(&source)?;
    writeln!(file, "#NAME \"Словарь с дисковой сортировкой\"")?;
    for index in 0..articles {
        writeln!(
            file,
            "alias{index:04}\nsame\n [b]Значение {index}[/b] [ref]alias0000[/ref]"
        )?;
        if index == 0 {
            writeln!(file, " [ref]missing[/ref] [s]media/image.png[/s]")?;
        }
    }
    let resource = root.join("image.bin");
    let mut output = File::create(&resource)?;
    for _ in 0..300 {
        output.write_all(&[137; 4096])?;
    }
    Ok(ConversionRequest {
        bundle: InputBundle {
            main: PreparedFile {
                id: "main".into(),
                display_name: "source.dsl".into(),
                path: source,
            },
            abbreviations: None,
            resources: vec![ResourceRecord {
                key: "\\media\\image.png".into(),
                path: resource,
            }],
        },
        options: ReadOptions::default(),
        metadata_override: None,
        staging_dir: root.join("ready"),
    })
}

#[test]
fn конвертирует_через_несколько_проходов() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let request = fixture(temp.path(), 300)?;
    let source_before = Sha256::digest(std::fs::read(&request.bundle.main.path)?);
    let events = Arc::new(Mutex::new(Vec::new()));
    let recorded = events.clone();
    let mut ctx = RunContext::new(temp.path().join("work"));
    ctx.limits.sort_buffer_bytes = 4096;
    ctx.limits.merge_fan_in = 2;
    ctx.progress = Arc::new(move |event| {
        if let CoreEvent::Progress(progress) | CoreEvent::StageCompleted(progress) = event {
            recorded.lock().expect("Журнал прогресса").push(progress);
        }
    });
    let artifacts = convert_to_staging(&request, &ctx)?;
    assert!(artifacts.summary.sort_metrics.merge_passes >= 2);
    assert!(artifacts.summary.sort_metrics.peak_sort_bytes <= ctx.limits.sort_buffer_bytes);
    assert_eq!(artifacts.summary.source_articles, 300);
    assert_eq!(artifacts.summary.output_keys, 600);
    assert_eq!(artifacts.summary.resources, 2);
    assert_eq!(artifacts.summary.warnings, 1);
    let options = OpenOptions::new().with_checksum_policy(ChecksumPolicy::Verify);
    let dictionary = MdxFile::open_with_options(&artifacts.mdx, &options)?;
    let mut duplicates = Vec::new();
    let mut count = 0;
    for key in dictionary.keys() {
        let key = key?;
        count += 1;
        if key.key() == "same" {
            duplicates.push(
                dictionary
                    .entry_at(key.ordinal())?
                    .ok_or("Статья дубля")?
                    .text()
                    .to_owned(),
            );
        }
    }
    assert_eq!(count, 600);
    assert_eq!(duplicates.len(), 300);
    for (index, html) in duplicates.iter().enumerate() {
        assert!(html.contains(&format!("<strong>Значение {index}</strong>")));
    }
    let resources = MddFile::open_with_options(&artifacts.mdd, &options)?;
    let mut extracted = Vec::new();
    resources
        .lookup_span("\\media\\image.png")?
        .ok_or("Ресурс MDD")?
        .copy_to(&mut extracted)?;
    assert_eq!(
        Sha256::digest(&extracted),
        Sha256::digest(std::fs::read(&request.bundle.resources[0].path)?)
    );
    let mut css = Vec::new();
    resources
        .lookup_span("\\converter.css")?
        .ok_or("CSS внутри MDD")?
        .copy_to(&mut css)?;
    assert_eq!(css, CONVERTER_CSS.as_bytes());
    let report: serde_json::Value = serde_json::from_reader(File::open(&artifacts.report)?)?;
    assert_eq!(report["diagnostics"][0]["code"], "missing_reference");
    assert!(
        events
            .lock()
            .expect("Прогресс")
            .iter()
            .all(|e| e.total.is_none_or(|n| e.completed <= n))
    );
    assert_eq!(
        source_before,
        Sha256::digest(std::fs::read(&request.bundle.main.path)?)
    );
    validate_artifacts(&artifacts, &ctx)?;
    let mut bytes = std::fs::read(&artifacts.mdx)?;
    let last = bytes.last_mut().ok_or("Файл MDX пуст")?;
    *last ^= 1;
    std::fs::write(&artifacts.mdx, bytes)?;
    assert_eq!(
        validate_artifacts(&artifacts, &ctx)
            .expect_err("Повреждённый блок")
            .code,
        "invalid_mdict"
    );
    Ok(())
}

#[test]
fn отмена_сортировки_не_оставляет_готовый_результат()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let request = fixture(temp.path(), 300)?;
    let mut ctx = RunContext::new(temp.path().join("work"));
    ctx.limits.sort_buffer_bytes = 4096;
    ctx.limits.merge_fan_in = 2;
    let control = ctx.control.clone();
    ctx.progress = Arc::new(move |event| {
        if matches!(
            event,
            CoreEvent::Progress(StageProgress {
                stage: ConversionStage::Sort,
                ..
            })
        ) {
            control.cancel();
        }
    });
    assert_eq!(
        convert_to_staging(&request, &ctx)
            .expect_err("Отмена сортировки")
            .code,
        "cancelled"
    );
    assert!(!request.staging_dir.exists());
    assert!(request.bundle.main.path.exists());
    Ok(())
}

#[test]
fn ссылка_на_подстатью_использует_ее_поисковый_вариант()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let mut request = fixture(temp.path(), 1)?;
    request.bundle.resources.clear();
    std::fs::write(
        &request.bundle.main.path,
        "parent\n @child{label}(u)\n  Дочерняя статья\n @\n",
    )?;
    let artifacts = convert_to_staging(&request, &RunContext::new(temp.path().join("work")))?;
    assert_eq!(artifacts.summary.source_articles, 2);
    assert_eq!(artifacts.summary.output_keys, 3);
    assert_eq!(artifacts.summary.warnings, 0);
    let dictionary = MdxFile::open(&artifacts.mdx)?;
    assert!(
        dictionary
            .lookup("parent")?
            .ok_or("Основная статья")?
            .text()
            .contains("entry://child\"")
    );
    assert!(dictionary.lookup("childu")?.is_some());
    Ok(())
}
