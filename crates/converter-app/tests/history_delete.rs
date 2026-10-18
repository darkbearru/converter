mod support;
use converter_app::{JobManager, JobState};

#[tokio::test]
async fn экспорт_защищает_запись_от_удаления_и_повторного_экспорта()
-> Result<(), Box<dyn std::error::Error>> {
    use converter_app::storage::ExportChoice;
    use std::{sync::atomic::Ordering, time::Duration};
    let temp = tempfile::tempdir()?;
    let storage = support::TestStorage::new(temp.path());
    storage.block_export.store(true, Ordering::SeqCst);
    let manager = JobManager::open(temp.path().join("data"), storage.clone())?;
    let job = manager
        .create(support::local(&manager, temp.path(), "export.dsl").await)
        .await?;
    manager.start(&job.id).await?;
    support::wait(&manager, &job.id, &[JobState::Succeeded]).await;
    let other = manager.clone();
    let id = job.id.clone();
    let choice = ExportChoice::Folder(temp.path().to_owned());
    let first = tokio::spawn(async move { other.export(&id, choice).await });
    tokio::time::timeout(Duration::from_secs(5), storage.export_started.notified()).await?;
    assert_eq!(
        manager
            .delete(&job.id)
            .await
            .expect_err("Удаление заблокировано экспортом")
            .code,
        "job_busy"
    );
    let second = tokio::time::timeout(
        Duration::from_secs(1),
        manager.export(&job.id, ExportChoice::Folder(temp.path().to_owned())),
    )
    .await;
    storage.export_release.notify_one();
    first.await??;
    assert_eq!(
        second?.expect_err("Повторный экспорт заблокирован").code,
        "job_busy"
    );
    manager.delete(&job.id).await?;
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn удаление_истории_сохраняет_исходник_и_результаты_после_перезапуска()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let root = temp.path().join("data");
    let storage = support::TestStorage::new(temp.path());
    let manager = JobManager::open(root.clone(), storage.clone())?;
    let draft = support::local(&manager, temp.path(), "source.dsl").await;
    let job = manager.create(draft).await?;
    manager.start(&job.id).await?;
    support::wait(&manager, &job.id, &[JobState::Succeeded]).await;
    let result = manager.result_directory(&job.id)?;
    manager.delete(&job.id).await?;
    assert!(manager.list()?.is_empty());
    assert!(temp.path().join("source.dsl").is_file());
    for name in ["dictionary.mdx", "dictionary.mdd", "conversion-report.json"] {
        assert!(result.join(name).is_file());
    }
    assert!(!root.join("jobs").join(format!("{}.json", job.id)).exists());
    assert!(
        !root
            .join("jobs")
            .join(format!("{}.diagnostics.jsonl", job.id))
            .exists()
    );
    manager.shutdown().await?;
    drop(manager);
    let manager = JobManager::open(root, storage)?;
    assert!(manager.list()?.is_empty());
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn нельзя_удалить_задание_из_очереди() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let manager = JobManager::open(
        temp.path().join("data"),
        support::TestStorage::new(temp.path()),
    )?;
    let job = manager
        .create(support::local(&manager, temp.path(), "queued.dsl").await)
        .await?;
    assert_eq!(
        manager
            .delete(&job.id)
            .await
            .expect_err("Нельзя удалить очередь")
            .code,
        "job_not_finished"
    );
    assert_eq!(manager.list()?.len(), 1);
    manager.shutdown().await?;
    Ok(())
}
