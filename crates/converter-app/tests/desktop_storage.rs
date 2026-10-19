use converter_app::{
    storage::{desktop::DesktopStorage, *},
    *,
};
use converter_core::{model::*, pipeline::convert_to_staging};
use std::{path::Path, sync::Arc};
mod support;

#[cfg(target_os = "macos")]
#[tokio::test]
async fn публикация_снимает_скрытость_папки_и_файлов_сохраняя_остальные_флаги()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    use std::os::darwin::fs::MetadataExt;
    let temp = tempfile::tempdir()?;
    let chosen = temp.path().join("Выбранная папка");
    std::fs::create_dir(&chosen)?;
    let storage = DesktopStorage::open(temp.path().join("app"))?;
    let output = storage.grant_output(&chosen)?;
    let ready = artifacts(&storage, temp.path(), &output)?;
    let paths = [&ready.directory, &ready.mdx, &ready.mdd, &ready.report];
    // Воспроизводим флаги результата, обнаруженные в каталоге iCloud Desktop.
    assert!(
        std::process::Command::new("/usr/bin/chflags")
            .args(["-h", "hidden,nodump"])
            .args(paths)
            .status()?
            .success()
    );
    let mdx = std::fs::read(&ready.mdx)?;
    let result = storage
        .publish(
            ready,
            &output,
            &RunContext::new(temp.path().join("scratch")),
        )
        .await?;
    assert_eq!(
        result.directory.parent(),
        Some(chosen.canonicalize()?.as_path())
    );
    for path in std::iter::once(result.directory.clone()).chain(
        ["dictionary.mdx", "dictionary.mdd", "conversion-report.json"]
            .map(|n| result.directory.join(n)),
    ) {
        let flags = std::fs::metadata(&path)?.st_flags();
        assert_eq!(flags & 0x8000, 0, "Finder должен видеть {}", path.display());
        assert_ne!(flags & 1, 0, "Флаг nodump должен сохраниться");
    }
    assert_eq!(std::fs::read(result.directory.join("dictionary.mdx"))?, mdx);
    assert_eq!(
        std::fs::read_dir(temp.path().join("app/results"))?.count(),
        0
    );
    // Результат предыдущей версии восстанавливает видимость при открытии папки.
    assert!(
        std::process::Command::new("/usr/bin/chflags")
            .args(["-h", "hidden"])
            .arg(&result.directory)
            .arg(result.directory.join("dictionary.mdx"))
            .status()?
            .success()
    );
    storage.checked_result_directory(&result.directory)?;
    assert_eq!(std::fs::metadata(&result.directory)?.st_flags() & 0x8000, 0);
    assert_eq!(
        std::fs::metadata(result.directory.join("dictionary.mdx"))?.st_flags() & 0x8000,
        0
    );
    Ok(())
}

#[tokio::test]
async fn локальный_файл_и_url_сохраняются_в_выбранную_папку()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    use converter_test_support::http::{FixtureServer, Scenario};
    for remote in [false, true] {
        let temp = tempfile::tempdir()?;
        let root = temp.path().join("app");
        let chosen = temp.path().join("Мои словари");
        std::fs::create_dir(&chosen)?;
        let storage = Arc::new(DesktopStorage::open(root.clone())?);
        let manager = JobManager::open(root.clone(), storage.clone())?;
        let server = FixtureServer::spawn(
            Scenario::StableRange,
            b"#NAME \"Remote\"\nalpha\n\tBody\n".to_vec(),
        )
        .await;
        let mut draft = if remote {
            support::draft(support::remote(server.url()))
        } else {
            support::local(&manager, temp.path(), "local.dsl").await
        };
        draft.output = storage.grant_output(&chosen)?;
        draft.prepare_only = remote;
        let job = manager.create(draft).await?;
        manager.start(&job.id).await?;
        if remote {
            support::wait(&manager, &job.id, &[JobState::Ready]).await;
            manager.start(&job.id).await?;
        }
        support::wait(&manager, &job.id, &[JobState::Succeeded]).await;
        let directory = manager.result_directory(&job.id)?;
        assert_eq!(directory.parent(), Some(chosen.canonicalize()?.as_path()));
        for name in ["dictionary.mdx", "dictionary.mdd", "conversion-report.json"] {
            assert!(std::fs::metadata(directory.join(name))?.len() > 0);
        }
        assert_eq!(std::fs::read_dir(root.join("results"))?.count(), 0);
        manager.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn экспортирует_готовое_задание_после_очистки_и_перезапуска()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let root = temp.path().join("app");
    let storage = Arc::new(DesktopStorage::open(root.clone())?);
    let manager = JobManager::open(root.clone(), storage.clone())?;
    let job = manager
        .create(support::local(&manager, temp.path(), "ready.dsl").await)
        .await?;
    manager.start(&job.id).await?;
    support::wait(&manager, &job.id, &[JobState::Succeeded]).await;
    manager.shutdown().await?;
    drop(manager);
    assert!(!root.join("scratch").join(&job.id).exists());
    let destination = temp.path().join("Копии");
    std::fs::create_dir(&destination)?;
    let destination = destination.canonicalize()?;
    storage.grant_output(&destination)?;
    let manager = JobManager::open(root.clone(), storage)?;
    let result_before = manager.result_directory(&job.id)?;
    let exported = manager
        .export(&job.id, ExportChoice::Folder(destination))
        .await?;
    assert_eq!(
        exported.status,
        ExportStatus::Succeeded,
        "{:?}",
        exported.error
    );
    assert_eq!(exported.saved_files.len(), 3);
    for file in &exported.saved_files {
        let path = Path::new(file);
        assert_eq!(
            std::fs::read(path)?,
            std::fs::read(result_before.join(path.file_name().ok_or("Имя")?))?
        );
    }
    assert_eq!(manager.result_directory(&job.id)?, result_before);
    assert_eq!(std::fs::read_dir(root.join("exports"))?.count(), 0);
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn новое_назначение_получает_копию_до_удаления_исходного_staging()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let storage = DesktopStorage::open(temp.path().join("app"))?;
    let old = temp.path().join("старое");
    let new = temp.path().join("новое");
    std::fs::create_dir(&old)?;
    std::fs::create_dir(&new)?;
    let old_output = storage.grant_output(&old)?;
    let new_output = storage.grant_output(&new)?;
    let ready = artifacts(&storage, temp.path(), &old_output)?;
    let original = ready.directory.clone();
    let bytes = std::fs::read(&ready.mdx)?;
    let result = storage
        .publish(
            ready,
            &new_output,
            &RunContext::new(temp.path().join("scratch")),
        )
        .await?;
    assert!(
        original.exists(),
        "Старый ready нужен до долговременного сохранения задания"
    );
    assert!(result.directory.starts_with(new.canonicalize()?));
    assert_eq!(
        std::fs::read(result.directory.join("dictionary.mdx"))?,
        bytes
    );
    assert_eq!(std::fs::read_dir(&new)?.count(), 1);
    Ok(())
}

fn artifacts(
    storage: &DesktopStorage,
    root: &Path,
    output: &OutputChoice,
) -> std::result::Result<ConvertedArtifacts, Box<dyn std::error::Error>> {
    let source = root.join("источник.dsl");
    std::fs::write(&source, "#NAME \"Тест\"\nalpha\n\t[b]Статья[/b]\n")?;
    let request = ConversionRequest {
        bundle: InputBundle {
            includes: vec![],
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
