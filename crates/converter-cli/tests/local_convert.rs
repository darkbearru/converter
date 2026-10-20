use converter_core::{
    model::*,
    pipeline::{convert_to_staging, publish_local},
};
use sha2::{Digest, Sha256};
use std::{path::Path, process::Command, sync::Arc};

fn request(root: &Path) -> std::result::Result<ConversionRequest, Box<dyn std::error::Error>> {
    let source = root.join("словарь.dsl");
    std::fs::write(
        &source,
        "#NAME \"Тест\"\nfirst\n [b]Первая статья[/b]\nsecond\n [ref]first[/ref]\n",
    )?;
    Ok(ConversionRequest {
        bundle: InputBundle {
            annotation: None,
            includes: vec![],
            main: PreparedFile {
                id: "main".into(),
                display_name: "словарь.dsl".into(),
                path: source,
            },
            abbreviations: None,
            resources: vec![],
        },
        options: ReadOptions::default(),
        metadata_override: None,
        staging_dir: root.join("staging"),
    })
}

#[test]
fn cli_не_перезаписывает_результат() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let request = request(temp.path())?;
    let hash = Sha256::digest(std::fs::read(&request.bundle.main.path)?);
    let output = temp.path().join("output");
    let mut paths = Vec::new();
    for _ in 0..2 {
        let result = Command::new(env!("CARGO_BIN_EXE_converter-cli"))
            .arg("convert")
            .arg("--source")
            .arg(&request.bundle.main.path)
            .arg("--output")
            .arg(&output)
            .output()?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let summary: serde_json::Value = serde_json::from_slice(&result.stdout)?;
        let directory = std::path::PathBuf::from(
            summary["results"][0]["directory"]
                .as_str()
                .ok_or("Каталог результата")?,
        );
        assert!(directory.join("dictionary.mdx").is_file());
        assert!(directory.join("dictionary.mdd").is_file());
        assert!(directory.join("conversion-report.json").is_file());
        assert_eq!(summary["results"][0]["summary"]["source_articles"], 2);
        paths.push(directory);
    }
    assert_ne!(paths[0], paths[1]);
    assert_eq!(
        hash,
        Sha256::digest(std::fs::read(&request.bundle.main.path)?)
    );
    Ok(())
}

#[test]
fn отмена_не_публикует_половину() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let request = request(temp.path())?;
    let mut ctx = RunContext::new(temp.path().join("work"));
    let control = ctx.control.clone();
    ctx.progress = Arc::new(move |event| {
        if matches!(
            event,
            CoreEvent::StageCompleted(StageProgress {
                stage: ConversionStage::WriteMdx,
                ..
            })
        ) {
            control.cancel();
        }
    });
    assert_eq!(
        convert_to_staging(&request, &ctx)
            .expect_err("Остановка между MDX и MDD")
            .code,
        "cancelled"
    );
    assert!(!request.staging_dir.exists());
    let ctx = RunContext::new(temp.path().join("work"));
    let artifacts = convert_to_staging(&request, &ctx)?;
    let output = temp.path().join("output");
    std::fs::create_dir(&output)?;
    ctx.control.cancel();
    assert_eq!(
        publish_local(artifacts, &output, "словарь", &ctx)
            .expect_err("Отмена перед публикацией")
            .code,
        "cancelled"
    );
    assert!(request.staging_dir.is_dir());
    assert_eq!(std::fs::read_dir(output)?.count(), 0);
    Ok(())
}

#[test]
fn публикация_сохраняет_занятые_имена_и_не_меняет_готовые_файлы()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let request = request(temp.path())?;
    let ctx = RunContext::new(temp.path().join("work"));
    let artifacts = convert_to_staging(&request, &ctx)?;
    let output = temp.path().join("output");
    std::fs::create_dir(&output)?;
    std::fs::create_dir(output.join("словарь-converted"))?;
    std::fs::write(output.join("словарь-converted-1"), "Существующий файл")?;
    let path = publish_local(artifacts, &output, "словарь", &ctx)?;
    assert_eq!(
        path.file_name().and_then(|s| s.to_str()),
        Some("словарь-converted-2")
    );
    assert!(output.join("словарь-converted").is_dir());
    assert_eq!(
        std::fs::read_to_string(output.join("словарь-converted-1"))?,
        "Существующий файл"
    );
    assert!(!request.staging_dir.exists());
    Ok(())
}
