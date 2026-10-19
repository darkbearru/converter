use converter_app::jobs::progress::{ProgressGate, aggregate_downloads};
use converter_download::{DownloadProgress, TransferState};
use std::time::Duration;
mod support;
#[tokio::test]
async fn диагностика_читается_страницами_из_реальной_конвертации()
-> Result<(), Box<dyn std::error::Error>> {
    use converter_app::{storage::PlatformSelection, *};
    let temp = tempfile::tempdir()?;
    let manager = JobManager::open(
        temp.path().join("data"),
        support::TestStorage::new(temp.path()),
    )?;
    let path = temp.path().join("warnings.dsl");
    std::fs::write(&path, b"#NAME \"Warnings\"\none\n\t[s]one.png[/s]\ntwo\n\t[s]two.png[/s]\nthree\n\t[s]three.png[/s]\n")?;
    let imports = manager
        .import(PlatformSelection::LocalPaths(vec![path]))
        .await?;
    let job = manager
        .create(support::draft(SourceInput::Local {
            import_id: imports[0].import_id.clone(),
        }))
        .await?;
    manager.start(&job.id).await?;
    support::wait(&manager, &job.id, &[JobState::SucceededWithWarnings]).await;
    let page = manager.diagnostics(&job.id, 0, 2)?;
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.next_offset, Some(2));
    let second = manager.diagnostics(&job.id, 2, 2)?;
    assert_eq!(second.items.len(), 1);
    assert_ne!(second.items[0].message, page.items[0].message);
    manager.shutdown().await?;
    Ok(())
}
#[test]
fn смешанный_размер_не_дает_общего_процента() {
    let file = |id: &str, total| DownloadProgress {
        source_id: id.into(),
        state: TransferState::Downloading,
        received_bytes: 50,
        confirmed_bytes: 40,
        total_bytes: total,
        bytes_per_second: None,
        restart_reason: None,
        restart_message: None,
    };
    let progress = aggregate_downloads(vec![file("one", Some(100)), file("two", None)]);
    assert!(progress.total_bytes.is_none());
    assert!(progress.percent.is_none());
    assert_eq!(progress.received_bytes, 100);
}
#[test]
fn progress_ограничен_а_смена_состояния_немедленна() {
    let mut gate = ProgressGate::default();
    let mut count = 0;
    for ms in 0..1000 {
        if gate.should_emit(Duration::from_millis(ms), "converting/read", false) {
            count += 1;
        }
    }
    assert_eq!(count, 5);
    assert!(gate.should_emit(Duration::from_millis(999), "succeeded", true));
    assert!(gate.should_emit(Duration::from_millis(999), "download_paused", true));
}

#[tokio::test]
async fn события_ядра_не_затапливают_подписчика() -> Result<(), Box<dyn std::error::Error>> {
    use converter_app::*;
    use converter_core::model::*;
    let temp = tempfile::tempdir()?;
    let manager = JobManager::open(
        temp.path().join("data"),
        support::TestStorage::new(temp.path()),
    )?;
    let job = manager
        .create(support::local(&manager, temp.path(), "local.dsl").await)
        .await?;
    let mut events = manager.subscribe();
    let progress = |completed| StageProgress {
        stage: ConversionStage::Read,
        completed,
        total: Some(1000),
        unit: ProgressUnit::Entries,
        articles: completed,
        resources: 0,
    };
    for count in 0..500 {
        manager.accept_core_event(
            &job.id,
            job.generation,
            &job.run_id,
            CoreEvent::Progress(progress(count)),
        )?;
    }
    let mut count = 0;
    while events.try_recv().is_ok() {
        count += 1;
    }
    assert!(count <= 5, "{count} промежуточных событий");
    manager.accept_core_event(
        &job.id,
        job.generation,
        &job.run_id,
        CoreEvent::StageCompleted(progress(1000)),
    )?;
    assert_eq!(
        events
            .try_recv()?
            .snapshot
            .conversion
            .current
            .expect("stage")
            .completed,
        1000
    );
    manager.shutdown().await?;
    Ok(())
}
