mod support;

use converter_app::{ConversionStatus, JobManager, JobSnapshot, JobState};
use converter_core::model::{
    ConversionStage, CoreEvent, Diagnostic, DiagnosticSeverity, ProgressUnit, StageProgress,
};
use std::{sync::atomic::Ordering, time::Duration};

fn diagnostic(severity: DiagnosticSeverity) -> CoreEvent {
    CoreEvent::Diagnostic(Diagnostic {
        message_key: None,
        message_args: Default::default(),
        code: "missing_resource".into(),
        message: "Отсутствует ресурс: voice.wav".into(),
        source_location: None,
        severity,
        occurrences: 3,
    })
}

fn progress() -> StageProgress {
    StageProgress {
        stage: ConversionStage::Read,
        completed: 10,
        total: Some(10),
        unit: ProgressUnit::Entries,
        articles: 10,
        resources: 0,
    }
}

async fn wait_worker_cancelled(manager: &JobManager) -> JobSnapshot {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let snapshot = manager.list().expect("Очередь").remove(0);
            if snapshot
                .error
                .as_ref()
                .is_some_and(|error| error.code == "cancelled")
            {
                return snapshot;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("Worker завершился после отмены")
}

#[tokio::test]
async fn накопленное_предупреждение_доходит_после_отмены_и_сохраняется_один_раз()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let root = temp.path().join("data");
    let storage = support::TestStorage::new(temp.path());
    storage.block_publish.store(true, Ordering::SeqCst);
    let manager = JobManager::open(root.clone(), storage.clone())?;
    let job = manager
        .create(support::local(&manager, temp.path(), "source.dsl").await)
        .await?;
    let running = manager.start(&job.id).await?;
    tokio::time::timeout(Duration::from_secs(10), storage.publish_started.notified()).await?;
    let cancelled = manager.cancel(&job.id).await?;
    assert_eq!(cancelled.state, JobState::Cancelled);

    // Реальный worker ещё удерживает открытый журнал. Воспроизводим callback
    // накопленной группы предупреждений, который приходит при завершении ядра.
    let accepted = manager.accept_core_event(
        &job.id,
        running.generation,
        &running.run_id,
        diagnostic(DiagnosticSeverity::Warning),
    )?;
    storage.publish_release.notify_one();
    let stopped = wait_worker_cancelled(&manager).await;
    assert!(
        accepted,
        "Предупреждение текущего завершающегося запуска потеряно"
    );
    assert_eq!(stopped.state, JobState::Cancelled);
    assert_eq!(stopped.conversion.status, ConversionStatus::Cancelled);
    assert_eq!(stopped.diagnostic_count, 1);
    assert!(
        !manager.accept_core_event(
            &job.id,
            running.generation,
            &running.run_id,
            diagnostic(DiagnosticSeverity::Warning),
        )?,
        "Завершившийся worker больше не может дописывать журнал"
    );
    let page = manager.diagnostics(&job.id, 0, 50)?;
    assert_eq!(page.total, 1);
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].occurrences, 3);
    manager.shutdown().await?;
    drop(manager);

    let reopened = JobManager::open(root, storage)?;
    let restored = reopened.list()?.remove(0);
    assert_eq!(restored.state, JobState::Cancelled);
    assert_eq!(restored.diagnostic_count, 1);
    assert!(
        !reopened.accept_core_event(
            &job.id,
            running.generation,
            &running.run_id,
            diagnostic(DiagnosticSeverity::Warning),
        )?,
        "После перезапуска нет активного журнала worker"
    );
    assert_eq!(reopened.diagnostics(&job.id, 0, 50)?.items.len(), 1);
    reopened.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn отменённый_запуск_отклоняет_прогресс_информацию_и_чужие_события()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let storage = support::TestStorage::new(temp.path());
    storage.block_publish.store(true, Ordering::SeqCst);
    let manager = JobManager::open(temp.path().join("data"), storage.clone())?;
    let job = manager
        .create(support::local(&manager, temp.path(), "source.dsl").await)
        .await?;
    let running = manager.start(&job.id).await?;
    tokio::time::timeout(Duration::from_secs(10), storage.publish_started.notified()).await?;
    let cancelled = manager.cancel(&job.id).await?;
    for event in [
        CoreEvent::Progress(progress()),
        CoreEvent::StageCompleted(progress()),
        diagnostic(DiagnosticSeverity::Info),
    ] {
        assert!(!manager.accept_core_event(&job.id, running.generation, &running.run_id, event)?);
    }
    assert!(!manager.accept_core_event(
        &job.id,
        running.generation + 1,
        &running.run_id,
        diagnostic(DiagnosticSeverity::Warning),
    )?);
    assert!(!manager.accept_core_event(
        &job.id,
        running.generation,
        "другой-запуск",
        diagnostic(DiagnosticSeverity::Warning),
    )?);
    let unchanged = manager.list()?.remove(0);
    assert_eq!(unchanged.state, JobState::Cancelled);
    assert_eq!(unchanged.conversion.status, ConversionStatus::Cancelled);
    assert_eq!(unchanged.sequence, cancelled.sequence);
    assert_eq!(unchanged.diagnostic_count, 0);
    assert!(manager.diagnostics(&job.id, 0, 50)?.items.is_empty());
    storage.publish_release.notify_one();
    wait_worker_cancelled(&manager).await;
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn повторный_запуск_не_принимает_предупреждения_от_старого_worker()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let storage = support::TestStorage::new(temp.path());
    storage.block_publish.store(true, Ordering::SeqCst);
    let manager = JobManager::open(temp.path().join("data"), storage.clone())?;
    let job = manager
        .create(support::local(&manager, temp.path(), "source.dsl").await)
        .await?;
    let old = manager.start(&job.id).await?;
    tokio::time::timeout(Duration::from_secs(10), storage.publish_started.notified()).await?;
    manager.cancel(&job.id).await?;
    storage.publish_release.notify_one();
    wait_worker_cancelled(&manager).await;
    let retry = manager.start(&job.id).await?;
    tokio::time::timeout(Duration::from_secs(10), storage.publish_started.notified()).await?;
    assert!(retry.generation > old.generation);
    assert_ne!(retry.run_id, old.run_id);
    for (generation, run_id) in [
        (old.generation, old.run_id.as_str()),
        (old.generation, retry.run_id.as_str()),
        (retry.generation, old.run_id.as_str()),
    ] {
        assert!(!manager.accept_core_event(
            &job.id,
            generation,
            run_id,
            diagnostic(DiagnosticSeverity::Warning),
        )?);
    }
    assert_eq!(manager.list()?.remove(0).diagnostic_count, 0);
    assert!(manager.diagnostics(&job.id, 0, 50)?.items.is_empty());
    storage.publish_release.notify_one();
    let ready = support::wait(&manager, &job.id, &[JobState::Succeeded]).await;
    assert_eq!(ready.diagnostic_count, 0);
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn отмена_без_запущенного_worker_не_открывает_приём_диагностики()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let manager = JobManager::open(
        temp.path().join("data"),
        support::TestStorage::new(temp.path()),
    )?;
    let job = manager
        .create(support::local(&manager, temp.path(), "source.dsl").await)
        .await?;
    manager.cancel(&job.id).await?;
    assert!(!manager.accept_core_event(
        &job.id,
        job.generation,
        &job.run_id,
        diagnostic(DiagnosticSeverity::Warning),
    )?);
    assert_eq!(manager.list()?.remove(0).diagnostic_count, 0);
    assert!(manager.diagnostics(&job.id, 0, 50)?.items.is_empty());
    manager.shutdown().await?;
    Ok(())
}
