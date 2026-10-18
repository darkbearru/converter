mod support;
#[tokio::test]
async fn отмена_согласует_статусы_скачивания_и_сохраняет_checkpoint()
-> Result<(), Box<dyn std::error::Error>> {
    use converter_download::TransferState;
    let temp = tempfile::tempdir()?;
    let manager = JobManager::open(
        temp.path().join("data"),
        support::TestStorage::new(temp.path()),
    )?;
    let server = FixtureServer::spawn(Scenario::HoldAt(1_048_576), vec![5; 2_097_152]).await;
    let job = manager
        .create(support::draft(support::remote(server.url())))
        .await?;
    manager.start(&job.id).await?;
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if manager.list().expect("jobs")[0].download.confirmed_bytes >= 1_048_576 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await?;
    let cancelled = manager.cancel(&job.id).await?;
    assert_eq!(cancelled.download.files[0].state, TransferState::Cancelled);
    assert!(cancelled.download.files[0].bytes_per_second.is_none());
    assert_eq!(cancelled.download.confirmed_bytes, 1_048_576);
    manager.shutdown().await?;
    let cancelled = manager.list()?.remove(0);
    assert_eq!(cancelled.state, JobState::Cancelled);
    assert_eq!(cancelled.download.files[0].state, TransferState::Cancelled);
    assert_eq!(cancelled.download.confirmed_bytes, 1_048_576);
    assert!(cancelled.result.is_none());
    Ok(())
}
#[tokio::test]
async fn поздняя_отмена_не_отменяет_уже_готовый_результат() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    let manager = JobManager::open(
        temp.path().join("data"),
        support::TestStorage::new(temp.path()),
    )?;
    let job = manager
        .create(support::local(&manager, temp.path(), "ready.dsl").await)
        .await?;
    manager.start(&job.id).await?;
    let ready = support::wait(&manager, &job.id, &[JobState::Succeeded]).await;
    let after = manager.cancel(&job.id).await?;
    assert_eq!(after.state, JobState::Succeeded);
    assert_eq!(
        after.result.expect("result").id,
        ready.result.expect("ready").id
    );
    manager.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn неоднозначные_ресурсы_ожидают_назначения() -> Result<(), Box<dyn std::error::Error>> {
    use converter_app::storage::PlatformSelection;
    let temp = tempfile::tempdir()?;
    let manager = JobManager::open(
        temp.path().join("data"),
        support::TestStorage::new(temp.path()),
    )?;
    let mut draft = support::local(&manager, temp.path(), "local.dsl").await;
    std::fs::write(
        temp.path().join("local.dsl"),
        b"#NAME \"Fixture\"\nalpha\n\t[s]picture.png[/s]\n",
    )?;
    let mut ids = Vec::new();
    for index in 0..2 {
        let dir = temp.path().join(format!("resources-{index}"));
        std::fs::create_dir(&dir)?;
        let path = dir.join("picture.png");
        std::fs::write(&path, [index as u8])?;
        let imported = manager
            .import(PlatformSelection::LocalPaths(vec![path]))
            .await?;
        ids.push(imported[0].import_id.clone());
        draft.resources.push(SourceInput::Local {
            import_id: imported[0].import_id.clone(),
        });
    }
    let job = manager.create(draft).await?;
    manager.start(&job.id).await?;
    support::wait(&manager, &job.id, &[JobState::AwaitingInput]).await;
    manager
        .update(
            &job.id,
            JobOptionsUpdate {
                resources: Some(vec![ids.remove(0)]),
                ..Default::default()
            },
        )
        .await?;
    manager.start(&job.id).await?;
    support::wait(&manager, &job.id, &[JobState::Succeeded]).await;
    manager.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn отчет_содержит_происхождение_без_url_токенов() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let storage = support::TestStorage::new(temp.path());
    let manager = JobManager::open(temp.path().join("data"), storage)?;
    let payload = b"#NAME \"Fixture\"\nalpha\n\tBody\n".to_vec();
    let bytes = payload.len() as u64;
    let server = FixtureServer::spawn(Scenario::StableRange, payload).await;
    let job = manager
        .create(support::draft(support::remote(format!(
            "{}?secret-token",
            server.url()
        ))))
        .await?;
    manager.start(&job.id).await?;
    support::wait(&manager, &job.id, &[JobState::Succeeded]).await;
    let text = std::fs::read_to_string(
        manager
            .result_directory(&job.id)?
            .join("conversion-report.json"),
    )?;
    let report: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(report["application"]["version"], "0.1.0");
    assert_eq!(report["sources"][0]["bytes"], bytes);
    assert_eq!(
        report["sources"][0]["sha256"].as_str().expect("hash").len(),
        64
    );
    assert_eq!(report["sources"][0]["statistics"]["resumes"], 0);
    assert!(!text.contains("secret-token"));
    manager.shutdown().await?;
    Ok(())
}
use converter_app::*;
use converter_test_support::http::{FixtureServer, Scenario};
use std::{sync::atomic::Ordering, time::Duration};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn не_конвертирует_неполный_источник() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let storage = support::TestStorage::new(temp.path());
    let manager = JobManager::open(temp.path().join("data"), storage.clone())?;
    let main = FixtureServer::spawn(
        Scenario::StableRange,
        b"#NAME \"Fixture\"\nalpha\n\tBody\n".to_vec(),
    )
    .await;
    let attachment = FixtureServer::spawn(Scenario::HoldAt(0), vec![3; 1000]).await;
    let mut draft = support::draft(support::remote(format!("{}?secret-token", main.url())));
    draft.resources.push(support::remote(attachment.url()));
    let job = manager.create(draft).await?;
    manager.start(&job.id).await?;
    tokio::time::timeout(Duration::from_secs(10), async {
        while attachment.requests().is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    assert_eq!(storage.invocations.load(Ordering::SeqCst), 0);
    assert!(!serde_json::to_string(&manager.list()?)?.contains("secret-token"));
    manager.pause_download(&job.id).await?;
    support::wait(&manager, &job.id, &[JobState::DownloadPaused]).await;
    let local = manager
        .create(support::local(&manager, temp.path(), "local.dsl").await)
        .await?;
    manager.start(&local.id).await?;
    support::wait(&manager, &local.id, &[JobState::Succeeded]).await;
    assert_eq!(storage.invocations.load(Ordering::SeqCst), 1);
    manager.cancel(&job.id).await?;
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn пауза_не_блокирует_очередь() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let storage = support::TestStorage::new(temp.path());
    let manager = JobManager::open(temp.path().join("data"), storage.clone())?;
    let mut ids = Vec::new();
    for index in 0..3 {
        let job = manager
            .create(support::local(&manager, temp.path(), &format!("local-{index}.dsl")).await)
            .await?;
        manager.start(&job.id).await?;
        ids.push(job.id);
    }
    for id in &ids {
        support::wait(&manager, id, &[JobState::Succeeded]).await;
    }
    assert_eq!(storage.peak.load(Ordering::SeqCst), 1);
    let first = manager.list()?.remove(0);
    assert_eq!(first.metadata.as_ref().expect("metadata").title, "Fixture");
    let job = manager.list()?.remove(0);
    let older = job.generation;
    manager.start(&job.id).await?;
    let ready = support::wait(&manager, &job.id, &[JobState::Succeeded]).await;
    assert!(ready.generation > older);
    let event = converter_core::model::CoreEvent::Progress(converter_core::model::StageProgress {
        stage: converter_core::model::ConversionStage::Read,
        completed: 0,
        total: None,
        unit: converter_core::model::ProgressUnit::Bytes,
        articles: 0,
        resources: 0,
    });
    assert!(!manager.accept_core_event(&job.id, older, &job.run_id, event)?);
    assert_eq!(
        manager
            .list()?
            .into_iter()
            .find(|s| s.id == job.id)
            .expect("job")
            .state,
        JobState::Succeeded
    );
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn ошибка_публикации_не_требует_новой_конвертации() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    let storage = support::TestStorage::new(temp.path());
    storage.fail_publish.store(true, Ordering::SeqCst);
    let manager = JobManager::open(temp.path().join("data"), storage.clone())?;
    let job = manager
        .create(support::local(&manager, temp.path(), "local.dsl").await)
        .await?;
    std::fs::write(
        temp.path().join("local.dsl"),
        b"#NAME \"Fixture\"\nalpha\n\t[s]missing.png[/s]\n",
    )?;
    manager.start(&job.id).await?;
    support::wait(&manager, &job.id, &[JobState::AwaitingInput]).await;
    manager
        .update(
            &job.id,
            JobOptionsUpdate {
                output: Some(OutputChoice::AppLibrary),
                ..Default::default()
            },
        )
        .await?;
    manager.start(&job.id).await?;
    support::wait(&manager, &job.id, &[JobState::SucceededWithWarnings]).await;
    assert_eq!(storage.invocations.load(Ordering::SeqCst), 1);
    let ready = manager.list()?[0].clone();
    assert_eq!(ready.diagnostic_count, 1);
    assert_eq!(manager.diagnostics(&job.id, 0, 10)?.items.len(), 1);
    let export = manager
        .export(
            &job.id,
            converter_app::storage::ExportChoice::DocumentHandle("unavailable".into()),
        )
        .await?;
    assert_eq!(export.status, ExportStatus::Failed);
    assert_eq!(
        manager.list()?[0].result.as_ref().expect("result").id,
        ready.result.expect("ready").id
    );
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn пакет_создает_отдельное_задание_для_каждого_словаря()
-> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let temp = tempfile::tempdir()?;
    let storage = support::TestStorage::new(temp.path());
    let manager = JobManager::open(temp.path().join("data"), storage)?;
    let path = temp.path().join("bundle.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path)?);
    for name in ["one.dsl", "two.dsl"] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())?;
        zip.write_all(b"#NAME \"Fixture\"\nalpha\n\tBody\n")?;
    }
    zip.finish()?;
    let imports = manager
        .import(converter_app::storage::PlatformSelection::LocalPaths(vec![
            path,
        ]))
        .await?;
    let job = manager
        .create(support::draft(SourceInput::Local {
            import_id: imports[0].import_id.clone(),
        }))
        .await?;
    manager.start(&job.id).await?;
    let waiting = support::wait(&manager, &job.id, &[JobState::AwaitingInput]).await;
    assert_eq!(waiting.input_choices.len(), 2);
    manager
        .update(
            &job.id,
            JobOptionsUpdate {
                dictionary_id: Some("*".into()),
                options: Some(converter_core::model::ReadOptions {
                    encoding: converter_core::model::EncodingChoice::Manual("utf-8".into()),
                }),
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(manager.list()?.len(), 2);
    assert!(manager.list()?.iter().all(|job| matches!(
        job.options.encoding,
        converter_core::model::EncodingChoice::Manual(_)
    )));
    manager.start(&job.id).await?;
    for job in manager.list()? {
        support::wait(&manager, &job.id, &[JobState::Succeeded]).await;
    }
    manager.shutdown().await?;
    Ok(())
}
