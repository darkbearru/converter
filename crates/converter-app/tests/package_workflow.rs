mod support;

use converter_app::{storage::PlatformSelection, *};
use std::{io::Write, path::Path, sync::atomic::Ordering, time::Duration};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn reorder_archive(path: &Path) -> TestResult {
    let mut archive = zip::ZipWriter::new(std::fs::File::create(path)?);
    for (name, contents) in [
        (
            "phrasebook.dsl",
            "#NAME \"PhraseBook\"\nphrase\n\tСамостоятельная статья\n",
        ),
        (
            "index.dsl",
            "#NAME \"Index\"\nindex\n\t[ref dict=\"Universal\"]alpha[/ref]\n",
        ),
        (
            "universal.dsl",
            "#NAME \"Universal\"\nalpha\n\tОсновная статья\n",
        ),
    ] {
        archive.start_file(name, zip::write::SimpleFileOptions::default())?;
        archive.write_all(contents.as_bytes())?;
    }
    archive.finish()?;
    Ok(())
}

#[tokio::test]
async fn замена_архива_не_подменяет_предложенный_словарь_и_основной_при_повторе() -> TestResult {
    for accept_before_change in [false, true] {
        let temp = tempfile::tempdir()?;
        let storage = support::TestStorage::new(temp.path());
        storage.block_publish.store(true, Ordering::SeqCst);
        let manager = JobManager::open(temp.path().join("data"), storage.clone())?;
        let parent = manager.create(package(&manager, temp.path()).await).await?;
        manager.start(&parent.id).await?;
        tokio::time::timeout(Duration::from_secs(10), storage.publish_started.notified()).await?;
        let offer = manager
            .list()?
            .remove(0)
            .additional_dictionaries
            .ok_or("Предложение")?;
        let queued = if accept_before_change {
            Some(
                manager
                    .resolve_additional(&parent.id, &offer.id, true)
                    .await?
                    .remove(0),
            )
        } else {
            None
        };
        reorder_archive(&temp.path().join("chinese.zip"))?;
        storage.block_publish.store(false, Ordering::SeqCst);
        storage.publish_release.notify_one();
        settled(&manager, &parent.id).await;
        let child = match queued {
            Some(child) => child,
            None => manager
                .resolve_additional(&parent.id, &offer.id, true)
                .await?
                .remove(0),
        };
        let child = settled(&manager, &child.id).await;
        assert_eq!(child.state, JobState::Failed);
        assert_eq!(
            child.error.as_ref().ok_or("Ошибка подмены")?.code,
            "source_changed"
        );
        assert!(child.result.is_none());
        manager.start(&parent.id).await?;
        let retry = settled(&manager, &parent.id).await;
        assert_eq!(retry.state, JobState::Failed);
        assert_eq!(
            retry.error.as_ref().ok_or("Ошибка повтора")?.code,
            "source_changed"
        );
        manager.shutdown().await?;
    }
    Ok(())
}

async fn package(manager: &JobManager, root: &Path) -> JobDraft {
    package_with_extra(manager, root, false).await
}

async fn package_with_extra(manager: &JobManager, root: &Path, extra: bool) -> JobDraft {
    let path = root.join("chinese.zip");
    let mut archive = zip::ZipWriter::new(std::fs::File::create(&path).expect("Архив"));
    let mut dictionaries = vec![
        (
            "universal.dsl",
            "#NAME \"Universal\"\nalpha\n\tОсновная статья\n",
        ),
        (
            "index.dsl",
            "#NAME \"Index\"\nindex\n\t[ref dict=\"Universal\"]alpha[/ref]\n",
        ),
        (
            "phrasebook.dsl",
            "#NAME \"PhraseBook\"\nphrase\n\tСамостоятельная статья\n",
        ),
    ];
    if extra {
        dictionaries.push(("notes.dsl", "#NAME \"Notes\"\nnote\n\tТретья статья\n"));
    }
    for (name, contents) in dictionaries {
        archive
            .start_file(name, zip::write::SimpleFileOptions::default())
            .expect("Файл архива");
        archive.write_all(contents.as_bytes()).expect("DSL");
    }
    archive.finish().expect("Закрыть архив");
    let source = manager
        .import(PlatformSelection::LocalPaths(vec![path]))
        .await
        .expect("Импорт")
        .remove(0);
    support::draft(SourceInput::Local {
        import_id: source.import_id,
    })
}

async fn settled(manager: &JobManager, id: &str) -> JobSnapshot {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let current = manager
                .list()
                .expect("Очередь")
                .into_iter()
                .find(|job| job.id == id)
                .expect("Задание");
            if matches!(
                current.state,
                JobState::Succeeded
                    | JobState::SucceededWithWarnings
                    | JobState::AwaitingInput
                    | JobState::Failed
            ) {
                return current;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("Завершение задания")
}

#[tokio::test]
async fn основной_словарь_конвертируется_без_ожидания_и_предлагает_независимый() -> TestResult {
    let temp = tempfile::tempdir()?;
    let data = temp.path().join("data");
    let manager = JobManager::open(data.clone(), support::TestStorage::new(temp.path()))?;
    let mut events = manager.subscribe();
    let parent = manager.create(package(&manager, temp.path()).await).await?;
    manager.start(&parent.id).await?;
    let result = settled(&manager, &parent.id).await;
    assert!(matches!(
        result.state,
        JobState::Succeeded | JobState::SucceededWithWarnings
    ));
    assert_eq!(
        result.metadata.as_ref().ok_or("Метаданные")?.title,
        "Universal"
    );
    let snapshot = serde_json::to_value(&result)?;
    assert_eq!(
        snapshot["additional_dictionaries"]["main_name"],
        "Universal"
    );
    let choices = snapshot["additional_dictionaries"]["dictionaries"]
        .as_array()
        .ok_or("Дополнительные словари")?;
    assert_eq!(choices.len(), 1);
    assert_eq!(choices[0]["display_name"], "phrasebook.dsl");
    assert_eq!(manager.list()?.len(), 1);
    let stored: serde_json::Value = serde_json::from_slice(&std::fs::read(
        data.join("jobs").join(format!("{}.json", parent.id)),
    )?)?;
    assert!(stored["selected_main"].as_str().is_some());
    assert_ne!(stored["selected_main"], choices[0]["id"]);
    let mut inspected = false;
    while let Ok(event) = events.try_recv() {
        inspected |= event.snapshot.state == JobState::Inspecting;
        assert_ne!(event.snapshot.state, JobState::AwaitingInput);
    }
    assert!(inspected);
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn согласие_во_время_основной_конвертации_создает_отдельный_словарь() -> TestResult {
    let temp = tempfile::tempdir()?;
    let storage = support::TestStorage::new(temp.path());
    storage.block_publish.store(true, Ordering::SeqCst);
    let manager = JobManager::open(temp.path().join("data"), storage.clone())?;
    let mut draft = package(&manager, temp.path()).await;
    draft.options.encoding = converter_core::model::EncodingChoice::Manual("utf-8".into());
    draft.metadata_override = Some(converter_core::model::DictionaryMetadata {
        title: "Название основного".into(),
        ..Default::default()
    });
    let output = temp.path().join("output");
    std::fs::create_dir(&output)?;
    draft.output = OutputChoice::DesktopFolder(output.clone());
    let resource = temp.path().join("cover.png");
    std::fs::write(&resource, b"image")?;
    let imported = manager
        .import(PlatformSelection::LocalPaths(vec![resource]))
        .await?
        .remove(0);
    draft.resources.push(SourceInput::Local {
        import_id: imported.import_id.clone(),
    });
    let parent = manager.create(draft).await?;
    manager.start(&parent.id).await?;
    tokio::time::timeout(Duration::from_secs(10), storage.publish_started.notified()).await?;
    let running = manager.list()?.remove(0);
    assert_eq!(running.state, JobState::Publishing);
    let offer = running.additional_dictionaries.ok_or("Предложение")?;
    let (first, second) = tokio::join!(
        manager.resolve_additional(&parent.id, &offer.id, true),
        manager.resolve_additional(&parent.id, &offer.id, true)
    );
    let child = first?.remove(0);
    assert_eq!(child.id, second?.remove(0).id);
    assert_ne!(child.id, parent.id);
    assert!(child.result.is_none());
    assert!(child.metadata.is_none());
    assert!(child.additional_dictionaries.is_none());
    assert_eq!(child.diagnostic_count, 0);
    assert!(!child.had_errors);
    assert_eq!(child.export.status, ExportStatus::NotStarted);
    assert_eq!(child.resource_attachments[0].source_id, imported.import_id);
    assert!(
        matches!(child.options.encoding, converter_core::model::EncodingChoice::Manual(ref name) if name == "utf-8")
    );
    assert!(
        manager
            .list()?
            .iter()
            .all(|job| job.additional_dictionaries.is_none())
    );
    assert_eq!(manager.list()?.len(), 2);
    storage.block_publish.store(false, Ordering::SeqCst);
    storage.publish_release.notify_one();
    let main_result = settled(&manager, &parent.id).await;
    let child_result = settled(&manager, &child.id).await;
    assert_eq!(
        main_result.metadata.ok_or("Основной")?.title,
        "Название основного"
    );
    assert_eq!(
        child_result.metadata.as_ref().ok_or("Дочерний")?.title,
        "PhraseBook"
    );
    assert!(matches!(
        child_result.state,
        JobState::Succeeded | JobState::SucceededWithWarnings
    ));
    assert_ne!(
        main_result.result.ok_or("Результат основного")?.id,
        child_result
            .result
            .as_ref()
            .ok_or("Результат дочернего")?
            .id
    );
    assert!(
        manager
            .result_directory(&child.id)?
            .starts_with(output.canonicalize()?)
    );
    let reused = manager
        .resolve_additional(&parent.id, &offer.id, true)
        .await?
        .remove(0);
    assert_eq!(reused.generation, child_result.generation);
    assert_eq!(storage.invocations.load(Ordering::SeqCst), 2);
    assert_eq!(storage.peak.load(Ordering::SeqCst), 1);
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn отказ_сохраняется_после_перезапуска_и_не_останавливает_основной() -> TestResult {
    let temp = tempfile::tempdir()?;
    let data = temp.path().join("data");
    let storage = support::TestStorage::new(temp.path());
    storage.block_publish.store(true, Ordering::SeqCst);
    let manager = JobManager::open(data.clone(), storage.clone())?;
    let parent = manager.create(package(&manager, temp.path()).await).await?;
    manager.start(&parent.id).await?;
    tokio::time::timeout(Duration::from_secs(10), storage.publish_started.notified()).await?;
    let offer = manager
        .list()?
        .remove(0)
        .additional_dictionaries
        .ok_or("Предложение")?;
    assert!(
        manager
            .resolve_additional(&parent.id, &offer.id, false)
            .await?
            .is_empty()
    );
    storage.block_publish.store(false, Ordering::SeqCst);
    storage.publish_release.notify_one();
    assert!(matches!(
        settled(&manager, &parent.id).await.state,
        JobState::Succeeded | JobState::SucceededWithWarnings
    ));
    manager.shutdown().await?;
    drop(manager);
    let manager = JobManager::open(data, storage)?;
    assert!(manager.list()?[0].additional_dictionaries.is_none());
    assert!(
        manager
            .resolve_additional(&parent.id, &offer.id, true)
            .await?
            .is_empty()
    );
    assert_eq!(manager.list()?.len(), 1);
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn завершение_основного_и_перезапуск_не_теряют_предложение() -> TestResult {
    let temp = tempfile::tempdir()?;
    let data = temp.path().join("data");
    let storage = support::TestStorage::new(temp.path());
    let manager = JobManager::open(data.clone(), storage.clone())?;
    let parent = manager.create(package(&manager, temp.path()).await).await?;
    manager.start(&parent.id).await?;
    let ready = settled(&manager, &parent.id).await;
    let offer = ready.additional_dictionaries.ok_or("Предложение")?;
    manager.shutdown().await?;
    drop(manager);
    let manager = JobManager::open(data.clone(), storage.clone())?;
    assert_eq!(
        manager.list()?[0]
            .additional_dictionaries
            .as_ref()
            .ok_or("Предложение")?
            .id,
        offer.id
    );
    let child = manager
        .resolve_additional(&parent.id, &offer.id, true)
        .await?
        .remove(0);
    let result = settled(&manager, &child.id).await;
    assert_eq!(result.metadata.ok_or("Метаданные")?.title, "PhraseBook");
    manager.shutdown().await?;
    drop(manager);
    let manager = JobManager::open(data.clone(), storage.clone())?;
    assert_eq!(manager.list()?.len(), 2);
    assert!(
        manager
            .list()?
            .iter()
            .all(|job| job.additional_dictionaries.is_none())
    );
    assert_eq!(
        manager
            .resolve_additional(&parent.id, &offer.id, true)
            .await?[0]
            .id,
        child.id
    );
    manager.delete(&child.id).await?;
    assert!(
        manager
            .resolve_additional(&parent.id, &offer.id, true)
            .await?
            .is_empty()
    );
    manager.shutdown().await?;
    drop(manager);
    let manager = JobManager::open(data, storage)?;
    assert_eq!(manager.list()?.len(), 1);
    assert!(
        manager
            .resolve_additional(&parent.id, &offer.id, true)
            .await?
            .is_empty()
    );
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn устаревший_идентификатор_не_создает_заданий() -> TestResult {
    let temp = tempfile::tempdir()?;
    let manager = JobManager::open(
        temp.path().join("data"),
        support::TestStorage::new(temp.path()),
    )?;
    let parent = manager.create(package(&manager, temp.path()).await).await?;
    manager.start(&parent.id).await?;
    settled(&manager, &parent.id).await;
    let error = manager
        .resolve_additional(&parent.id, "unknown-offer", true)
        .await
        .err()
        .ok_or("Ошибка")?;
    assert_eq!(error.code, "additional_offer_not_found");
    assert_eq!(manager.list()?.len(), 1);
    assert!(manager.list()?[0].additional_dictionaries.is_some());
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn ошибка_записи_дочернего_задания_допускает_повтор_того_же_предложения() -> TestResult {
    let temp = tempfile::tempdir()?;
    let data = temp.path().join("data");
    let manager = JobManager::open(data.clone(), support::TestStorage::new(temp.path()))?;
    let parent = manager.create(package(&manager, temp.path()).await).await?;
    manager.start(&parent.id).await?;
    let offer = settled(&manager, &parent.id)
        .await
        .additional_dictionaries
        .ok_or("Предложение")?;
    let path = data.join("jobs").join(format!("{}.json", parent.id));
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    let child_id = saved["additional_dictionaries"]["children"][0]["job_id"]
        .as_str()
        .ok_or("UUID")?;
    let blocker = data.join("jobs").join(format!("{child_id}.json"));
    std::fs::create_dir(&blocker)?;
    let error = manager
        .resolve_additional(&parent.id, &offer.id, true)
        .await
        .err()
        .ok_or("Ошибка записи")?;
    assert_eq!(error.code, "store_write_failed");
    assert!(manager.list()?[0].additional_dictionaries.is_none());
    assert_eq!(manager.list()?.len(), 1);
    assert_eq!(
        manager
            .delete(&parent.id)
            .await
            .err()
            .ok_or("Незавершённая отправка")?
            .code,
        "job_busy"
    );
    std::fs::remove_dir(&blocker)?;
    let child = manager
        .resolve_additional(&parent.id, &offer.id, true)
        .await?
        .remove(0);
    assert_eq!(child.id, child_id);
    assert_eq!(
        settled(&manager, &child.id)
            .await
            .metadata
            .ok_or("Метаданные")?
            .title,
        "PhraseBook"
    );
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn принятое_решение_восстанавливает_недостающего_ребенка_единожды() -> TestResult {
    let temp = tempfile::tempdir()?;
    let data = temp.path().join("data");
    let storage = support::TestStorage::new(temp.path());
    let manager = JobManager::open(data.clone(), storage.clone())?;
    let parent = manager.create(package(&manager, temp.path()).await).await?;
    manager.start(&parent.id).await?;
    let offer = settled(&manager, &parent.id)
        .await
        .additional_dictionaries
        .ok_or("Предложение")?;
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(
        data.join("jobs").join(format!("{}.json", parent.id)),
    )?)?;
    let child_id = saved["additional_dictionaries"]["children"][0]["job_id"]
        .as_str()
        .ok_or("UUID")?;
    let blocker = data.join("jobs").join(format!("{child_id}.json"));
    std::fs::create_dir(&blocker)?;
    assert!(
        manager
            .resolve_additional(&parent.id, &offer.id, true)
            .await
            .is_err()
    );
    manager.shutdown().await?;
    drop(manager);
    std::fs::remove_dir(&blocker)?;
    let manager = JobManager::open(data.clone(), storage.clone())?;
    let restored = manager
        .list()?
        .into_iter()
        .find(|job| job.id == child_id)
        .ok_or("Дочерний")?;
    assert_eq!(restored.state, JobState::Interrupted);
    assert!(restored.metadata.is_none());
    assert!(
        manager
            .list()?
            .iter()
            .all(|job| job.additional_dictionaries.is_none())
    );
    assert_eq!(
        manager
            .resolve_additional(&parent.id, &offer.id, true)
            .await?[0]
            .id,
        child_id
    );
    assert_eq!(manager.list()?.len(), 2);
    manager.delete(child_id).await?;
    manager.shutdown().await?;
    drop(manager);
    let manager = JobManager::open(data, storage)?;
    assert_eq!(manager.list()?.len(), 1);
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn прерванная_выдача_нескольких_словарей_сохраняет_первого_и_восстанавливает_второго()
-> TestResult {
    let temp = tempfile::tempdir()?;
    let data = temp.path().join("data");
    let storage = support::TestStorage::new(temp.path());
    let manager = JobManager::open(data.clone(), storage.clone())?;
    let parent = manager
        .create(package_with_extra(&manager, temp.path(), true).await)
        .await?;
    manager.start(&parent.id).await?;
    let ready = settled(&manager, &parent.id).await;
    let offer = ready.additional_dictionaries.ok_or("Предложение")?;
    assert_eq!(offer.dictionaries.len(), 2);
    let path = data.join("jobs").join(format!("{}.json", parent.id));
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    let first = saved["additional_dictionaries"]["children"][0]["job_id"]
        .as_str()
        .ok_or("Первый UUID")?
        .to_owned();
    let second = saved["additional_dictionaries"]["children"][1]["job_id"]
        .as_str()
        .ok_or("Второй UUID")?
        .to_owned();
    let blocker = data.join("jobs").join(format!("{second}.json"));
    std::fs::create_dir(&blocker)?;
    assert!(
        manager
            .resolve_additional(&parent.id, &offer.id, true)
            .await
            .is_err()
    );
    assert_eq!(manager.list()?.len(), 2);
    assert_eq!(
        manager
            .list()?
            .iter()
            .find(|job| job.id == first)
            .ok_or("Первый ребёнок")?
            .generation,
        0
    );
    manager.cancel(&first).await?;
    assert_eq!(
        manager
            .delete(&first)
            .await
            .err()
            .ok_or("Защита журнала")?
            .code,
        "job_busy"
    );
    manager.shutdown().await?;
    drop(manager);
    std::fs::remove_dir(&blocker)?;
    let manager = JobManager::open(data, storage.clone())?;
    let jobs = manager.list()?;
    assert_eq!(jobs.len(), 3);
    let restored_first = jobs
        .iter()
        .find(|job| job.id == first)
        .ok_or("Первый ребёнок")?;
    assert_eq!(restored_first.state, JobState::Cancelled);
    let restored_second = jobs
        .iter()
        .find(|job| job.id == second)
        .ok_or("Второй ребёнок")?;
    assert_eq!(restored_second.state, JobState::Interrupted);
    assert_eq!(
        manager
            .resolve_additional(&parent.id, &offer.id, true)
            .await?
            .len(),
        2
    );
    manager.start(&first).await?;
    manager.start(&second).await?;
    for id in [&first, &second] {
        assert!(matches!(
            settled(&manager, id).await.state,
            JobState::Succeeded | JobState::SucceededWithWarnings
        ));
    }
    let mut titles = manager
        .list()?
        .into_iter()
        .filter(|job| job.id != parent.id)
        .map(|job| job.metadata.expect("Метаданные").title)
        .collect::<Vec<_>>();
    titles.sort();
    assert_eq!(titles, ["Notes", "PhraseBook"]);
    assert_eq!(storage.invocations.load(Ordering::SeqCst), 3);
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn ошибка_записи_решения_оставляет_предложение_доступным_для_повтора() -> TestResult {
    let temp = tempfile::tempdir()?;
    let data = temp.path().join("data");
    let manager = JobManager::open(data.clone(), support::TestStorage::new(temp.path()))?;
    let parent = manager.create(package(&manager, temp.path()).await).await?;
    manager.start(&parent.id).await?;
    let offer = settled(&manager, &parent.id)
        .await
        .additional_dictionaries
        .ok_or("Предложение")?;
    let path = data.join("jobs").join(format!("{}.json", parent.id));
    let backup = data.join("parent-backup.json");
    std::fs::rename(&path, &backup)?;
    std::fs::create_dir(&path)?;
    let error = manager
        .resolve_additional(&parent.id, &offer.id, true)
        .await
        .err()
        .ok_or("Ошибка записи")?;
    assert_eq!(error.code, "store_write_failed");
    assert_eq!(manager.list()?.len(), 1);
    assert_eq!(
        manager.list()?[0]
            .additional_dictionaries
            .as_ref()
            .ok_or("Предложение сохранено")?
            .id,
        offer.id
    );
    std::fs::remove_dir(&path)?;
    std::fs::rename(&backup, &path)?;
    let child = manager
        .resolve_additional(&parent.id, &offer.id, true)
        .await?
        .remove(0);
    assert!(matches!(
        settled(&manager, &child.id).await.state,
        JobState::Succeeded | JobState::SucceededWithWarnings
    ));
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn отмена_основного_не_отменяет_независимый_словарь_и_сохраняет_исходник() -> TestResult {
    let temp = tempfile::tempdir()?;
    let storage = support::TestStorage::new(temp.path());
    storage.block_publish.store(true, Ordering::SeqCst);
    let manager = JobManager::open(temp.path().join("data"), storage.clone())?;
    let parent = manager.create(package(&manager, temp.path()).await).await?;
    manager.start(&parent.id).await?;
    tokio::time::timeout(Duration::from_secs(10), storage.publish_started.notified()).await?;
    let cancelled = manager.cancel(&parent.id).await?;
    assert_eq!(cancelled.state, JobState::Cancelled);
    let offer = cancelled.additional_dictionaries.ok_or("Предложение")?;
    let child = manager
        .resolve_additional(&parent.id, &offer.id, true)
        .await?
        .remove(0);
    storage.block_publish.store(false, Ordering::SeqCst);
    storage.publish_release.notify_one();
    let ready = settled(&manager, &child.id).await;
    assert!(matches!(
        ready.state,
        JobState::Succeeded | JobState::SucceededWithWarnings
    ));
    assert_eq!(ready.metadata.ok_or("Метаданные")?.title, "PhraseBook");
    assert!(ready.error.is_none());
    assert!(!ready.had_errors);
    assert!(temp.path().join("chinese.zip").is_file());
    let result_path = manager.result_directory(&child.id)?;
    manager.delete(&child.id).await?;
    assert!(result_path.join("dictionary.mdx").is_file());
    assert!(temp.path().join("chinese.zip").is_file());
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn частично_сохраненный_ребенок_после_сбоя_показывает_прерывание_вместо_вечной_очереди()
-> TestResult {
    let temp = tempfile::tempdir()?;
    let data = temp.path().join("data");
    let storage = support::TestStorage::new(temp.path());
    let manager = JobManager::open(data.clone(), storage.clone())?;
    let parent = manager
        .create(package_with_extra(&manager, temp.path(), true).await)
        .await?;
    manager.start(&parent.id).await?;
    let offer = settled(&manager, &parent.id)
        .await
        .additional_dictionaries
        .ok_or("Предложение")?;
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(
        data.join("jobs").join(format!("{}.json", parent.id)),
    )?)?;
    let first = saved["additional_dictionaries"]["children"][0]["job_id"]
        .as_str()
        .ok_or("Первый UUID")?;
    let second = saved["additional_dictionaries"]["children"][1]["job_id"]
        .as_str()
        .ok_or("Второй UUID")?;
    let blocker = data.join("jobs").join(format!("{second}.json"));
    std::fs::create_dir(&blocker)?;
    assert!(
        manager
            .resolve_additional(&parent.id, &offer.id, true)
            .await
            .is_err()
    );
    assert_eq!(
        manager
            .list()?
            .iter()
            .find(|job| job.id == first)
            .ok_or("Первый ребёнок")?
            .state,
        JobState::Queued
    );
    manager.shutdown().await?;
    drop(manager);
    std::fs::remove_dir(&blocker)?;
    let manager = JobManager::open(data, storage)?;
    let restored = manager.list()?;
    assert_eq!(restored.len(), 3);
    for child in restored.iter().filter(|job| job.id != parent.id) {
        assert_eq!(child.state, JobState::Interrupted);
        assert_eq!(child.conversion.status, ConversionStatus::Interrupted);
    }
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn отказ_после_сохраненного_согласия_не_запускает_недостающих_детей() -> TestResult {
    let temp = tempfile::tempdir()?;
    let data = temp.path().join("data");
    let manager = JobManager::open(data.clone(), support::TestStorage::new(temp.path()))?;
    let parent = manager.create(package(&manager, temp.path()).await).await?;
    manager.start(&parent.id).await?;
    let offer = settled(&manager, &parent.id)
        .await
        .additional_dictionaries
        .ok_or("Предложение")?;
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(
        data.join("jobs").join(format!("{}.json", parent.id)),
    )?)?;
    let child_id = saved["additional_dictionaries"]["children"][0]["job_id"]
        .as_str()
        .ok_or("UUID")?;
    let blocker = data.join("jobs").join(format!("{child_id}.json"));
    std::fs::create_dir(&blocker)?;
    assert!(
        manager
            .resolve_additional(&parent.id, &offer.id, true)
            .await
            .is_err()
    );
    std::fs::remove_dir(&blocker)?;
    let declined = manager
        .resolve_additional(&parent.id, &offer.id, false)
        .await;
    manager.shutdown().await?;
    assert_eq!(
        declined.as_ref().err().map(|error| error.code.as_str()),
        Some("additional_offer_already_accepted")
    );
    assert_eq!(manager.list()?.len(), 1);
    assert!(!data.join("jobs").join(format!("{child_id}.json")).exists());
    Ok(())
}
