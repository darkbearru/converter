mod support;

use converter_app::{JobManager, JobState};

#[tokio::test]
async fn восстановление_оформления_и_отсутствие_файла_сокращений_не_портят_статус()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let manager = JobManager::open(
        temp.path().join("data"),
        support::TestStorage::new(temp.path()),
    )?;
    let draft = support::local(&manager, temp.path(), "source.dsl").await;
    std::fs::write(
        temp.path().join("source.dsl"),
        "#NAME \"Info\"\nword\n [b]a[i]b[/b]c[/i] [p]n.[/p]\n",
    )?;
    let job = manager.create(draft).await?;
    manager.start(&job.id).await?;
    let ready = support::wait(
        &manager,
        &job.id,
        &[JobState::Succeeded, JobState::SucceededWithWarnings],
    )
    .await;
    assert_eq!(ready.state, JobState::Succeeded);
    assert_eq!(ready.diagnostic_count, 0);
    assert!(manager.diagnostics(&job.id, 0, 50)?.items.is_empty());
    let report: serde_json::Value = serde_json::from_reader(std::fs::File::open(
        manager
            .result_directory(&job.id)?
            .join("conversion-report.json"),
    )?)?;
    assert_eq!(report["summary"]["source_articles"], 1);
    assert_eq!(report["summary"]["warnings"], 0);
    assert!(
        report["diagnostics"]
            .as_array()
            .expect("Подробный журнал")
            .iter()
            .any(|item| item["code"] == "repaired_markup" && item["severity"] == "info")
    );
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn повторные_ресурсы_дают_группы_с_пагинацией_и_сохраняются_после_перезапуска()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let root = temp.path().join("data");
    let storage = support::TestStorage::new(temp.path());
    let manager = JobManager::open(root.clone(), storage.clone())?;
    let draft = support::local(&manager, temp.path(), "source.dsl").await;
    std::fs::write(
        temp.path().join("source.dsl"),
        "#NAME \"Groups\"\none\n [s]voice.wav[/s] [s]image.png[/s]\ntwo\n [s]voice.wav[/s]\n",
    )?;
    let job = manager.create(draft).await?;
    manager.start(&job.id).await?;
    let ready = support::wait(&manager, &job.id, &[JobState::SucceededWithWarnings]).await;
    assert_eq!(ready.diagnostic_count, 2);
    let first = manager.diagnostics(&job.id, 0, 1)?;
    assert_eq!(first.total, 2);
    assert_eq!(first.next_offset, Some(1));
    assert_eq!(serde_json::to_value(&first.items[0])?["occurrences"], 2);
    let second = manager.diagnostics(&job.id, 1, 1)?;
    assert_eq!(second.next_offset, None);
    assert_eq!(second.items.len(), 1);
    assert!(second.items[0].message.contains("image.png"));
    manager.shutdown().await?;
    drop(manager);
    let reopened = JobManager::open(root, storage)?;
    assert_eq!(reopened.list()?[0].diagnostic_count, 2);
    let page = reopened.diagnostics(&job.id, 0, 50)?;
    assert_eq!(page.items.len(), 2);
    assert_eq!(serde_json::to_value(&page.items[0])?["occurrences"], 2);
    reopened.shutdown().await?;
    Ok(())
}
