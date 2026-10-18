mod support;
#[tokio::test]
async fn счетчики_скачивания_восстанавливаются_из_checkpoint()
-> Result<(), Box<dyn std::error::Error>> {
    use converter_test_support::http::{FixtureServer, Scenario};
    use std::time::Duration;
    let temp = tempfile::tempdir()?;
    let data = temp.path().join("data");
    let storage = support::TestStorage::new(temp.path());
    let server = FixtureServer::spawn(Scenario::HoldAt(1_048_576), vec![4; 2_097_152]).await;
    let manager = JobManager::open(data.clone(), storage.clone())?;
    let job = manager
        .create(support::draft(support::remote(server.url())))
        .await?;
    manager.start(&job.id).await?;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if manager.list().expect("jobs")[0].download.confirmed_bytes >= 1_048_576 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    let source = manager.list()?[0].download.files[0].source_id.clone();
    assert!(manager.remove_cached_source(&source).is_err());
    manager.shutdown().await?;
    drop(manager);
    let path = data.join("jobs").join(format!("{}.json", job.id));
    let mut value: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    value["snapshot"]["state"] = "downloading".into();
    value["snapshot"]["download"]["files"][0]["confirmed_bytes"] = 0.into();
    value["snapshot"]["download"]["files"][0]["received_bytes"] = 0.into();
    std::fs::write(path, serde_json::to_vec(&value)?)?;
    let manager = JobManager::open(data, storage)?;
    let reopened = manager.list()?.remove(0);
    assert_eq!(reopened.state, JobState::DownloadPaused);
    assert_eq!(reopened.download.confirmed_bytes, 1_048_576);
    assert_eq!(reopened.download.received_bytes, 1_048_576);
    manager.shutdown().await?;
    Ok(())
}
use converter_app::*;
#[tokio::test]
async fn перезапуск_сохраняет_результат_и_помечает_прерывание()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let data = temp.path().join("data");
    let storage = support::TestStorage::new(temp.path());
    let manager = JobManager::open(data.clone(), storage.clone())?;
    let job = manager
        .create(support::local(&manager, temp.path(), "local.dsl").await)
        .await?;
    manager.start(&job.id).await?;
    let ready = support::wait(&manager, &job.id, &[JobState::Succeeded]).await;
    manager.shutdown().await?;
    drop(manager);
    let manager = JobManager::open(data.clone(), storage.clone())?;
    assert_eq!(
        manager.list()?[0].result.as_ref().expect("result").id,
        ready.result.expect("ready").id
    );
    manager.shutdown().await?;
    drop(manager);
    let path = data.join("jobs").join(format!("{}.json", job.id));
    let mut value: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    value["snapshot"]["state"] = "converting".into();
    value["snapshot"]["conversion"]["status"] = "running".into();
    let staging =
        data.join("results")
            .join(format!(".converter-{}-{}", job.id, uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&staging)?;
    std::fs::write(staging.join("partial.mdx"), b"partial")?;
    value["staging_path"] = serde_json::to_value(&staging)?;
    std::fs::write(&path, serde_json::to_vec(&value)?)?;
    std::fs::create_dir_all(data.join("scratch").join(&job.id))?;
    std::fs::write(
        data.join("scratch").join(&job.id).join("partial"),
        b"partial",
    )?;
    let manager = JobManager::open(data.clone(), storage)?;
    assert_eq!(manager.list()?[0].state, JobState::Interrupted);
    assert!(!data.join("scratch").join(&job.id).exists());
    assert!(!staging.exists());
    manager.shutdown().await?;
    Ok(())
}
