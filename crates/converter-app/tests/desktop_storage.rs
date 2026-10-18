use converter_app::{
    storage::{desktop::DesktopStorage, *},
    *,
};
use converter_core::{model::*, pipeline::convert_to_staging};
use std::{path::Path, sync::Arc};

fn artifacts(
    storage: &DesktopStorage,
    root: &Path,
    output: &OutputChoice,
) -> std::result::Result<ConvertedArtifacts, Box<dyn std::error::Error>> {
    let source = root.join("источник.dsl");
    std::fs::write(&source, "#NAME \"Тест\"\nalpha\n\t[b]Статья[/b]\n")?;
    let request = ConversionRequest {
        bundle: InputBundle {
            main: PreparedFile {
                id: "main".into(),
                display_name: "источник.dsl".into(),
                path: source,
            },
            abbreviations: None,
            resources: vec![],
        },
        options: ReadOptions::default(),
        metadata_override: None,
        staging_dir: storage.create_staging("job-1", output)?,
    };
    Ok(convert_to_staging(
        &request,
        &RunContext::new(root.join("scratch")),
    )?)
}
#[tokio::test]
async fn публикует_в_папку_с_кириллицей() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let chosen = temp.path().join("Готовые словари");
    std::fs::create_dir(&chosen)?;
    let chosen = chosen.canonicalize()?;
    let storage = DesktopStorage::open(temp.path().join("app"))?;
    assert!(
        storage
            .create_staging("job-1", &OutputChoice::DesktopFolder(chosen.clone()))
            .is_err()
    );
    let output = storage.grant_output(&chosen)?;
    let ready = artifacts(&storage, temp.path(), &output)?;
    let before = std::fs::read(temp.path().join("источник.dsl"))?;
    std::fs::create_dir(chosen.join("Тест-converted"))?;
    let first = storage
        .publish(
            ready,
            &output,
            &RunContext::new(temp.path().join("scratch")),
        )
        .await?;
    let second = storage
        .publish(
            artifacts(&storage, temp.path(), &output)?,
            &output,
            &RunContext::new(temp.path().join("scratch")),
        )
        .await?;
    assert!(first.directory.starts_with(&chosen));
    assert_ne!(first.directory, second.directory);
    assert_eq!(
        first.directory.file_name().and_then(|s| s.to_str()),
        Some("Тест-converted-1")
    );
    assert!(chosen.join("Тест-converted").is_dir());
    assert_eq!(before, std::fs::read(temp.path().join("источник.dsl"))?);
    assert_eq!(std::fs::read_dir(first.directory)?.count(), 3);
    drop(storage);
    let storage = DesktopStorage::open(temp.path().join("app"))?;
    assert!(storage.create_staging("job-1", &output).is_ok());
    Ok(())
}
#[tokio::test]
async fn не_экспортирует_за_пределы_результата()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let storage = Arc::new(DesktopStorage::open(temp.path().join("app"))?);
    let output = OutputChoice::AppLibrary;
    let ready = artifacts(&storage, temp.path(), &output)?;
    let ctx = RunContext::new(temp.path().join("scratch"));
    let result = storage.publish(ready, &output, &ctx).await?;
    let destination = temp.path().join("Экспорт");
    std::fs::create_dir(&destination)?;
    let destination = destination.canonicalize()?;
    storage.grant_output(&destination)?;
    let first = storage
        .export(&result, ExportChoice::Folder(destination.clone()), &ctx)
        .await?;
    assert_eq!(first.status, ExportStatus::Succeeded);
    assert_eq!(first.saved_files.len(), 3);
    storage
        .export(&result, ExportChoice::Folder(destination.clone()), &ctx)
        .await?;
    assert_eq!(std::fs::read_dir(&destination)?.count(), 2);
    // Подмена содержимого результата ссылкой не должна копировать внешний файл.
    let outside = temp.path().join("secret");
    std::fs::write(&outside, b"secret")?;
    std::fs::remove_file(result.directory.join("dictionary.mdd"))?;
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, result.directory.join("dictionary.mdd"))?;
    #[cfg(windows)]
    std::fs::create_dir(result.directory.join("dictionary.mdd"))?;
    assert!(
        storage
            .export(&result, ExportChoice::Folder(destination.clone()), &ctx)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read_dir(destination)?.count(), 2);
    assert_eq!(std::fs::read(outside)?, b"secret");
    Ok(())
}
#[tokio::test]
async fn отмена_экспорта_не_показывает_частичный_результат()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let storage = DesktopStorage::open(temp.path().join("app"))?;
    let ctx = RunContext::new(temp.path().join("scratch"));
    let output = OutputChoice::AppLibrary;
    let result = storage
        .publish(artifacts(&storage, temp.path(), &output)?, &output, &ctx)
        .await?;
    let destination = temp.path().join("export");
    std::fs::create_dir(&destination)?;
    let destination = destination.canonicalize()?;
    storage.grant_output(&destination)?;
    ctx.control.cancel();
    assert!(
        storage
            .export(&result, ExportChoice::Folder(destination.clone()), &ctx)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read_dir(destination)?.count(), 0);
    Ok(())
}
