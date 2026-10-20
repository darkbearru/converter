mod support;
use converter_app::{storage::PlatformSelection, *};
use std::{path::Path, sync::atomic::Ordering};
type TestResult = Result<(), Box<dyn std::error::Error>>;

async fn drafts(manager: &JobManager, root: &Path, order: &[&str]) -> Vec<JobDraft> {
    manager
        .import(PlatformSelection::LocalPaths(
            order.iter().map(|name| root.join(name)).collect(),
        ))
        .await
        .expect("импорт")
        .into_iter()
        .map(|file| {
            support::draft(SourceInput::Local {
                import_id: file.import_id,
            })
        })
        .collect()
}
fn fixture(root: &Path) -> std::io::Result<()> {
    std::fs::write(
        root.join("main.dsl"),
        "#NAME \"Main ( 1 / 3 )\"\n#INCLUDE \"part.dsl\"\n#INCLUDE \"last.dsl\"\nroot\n [ref]part[/ref]\n",
    )?;
    std::fs::write(
        root.join("part.dsl"),
        "#NAME \"Main ( 2 / 3 )\"\npart\n [ref]root[/ref]\n",
    )?;
    std::fs::write(
        root.join("last.dsl"),
        "#NAME \"Main ( 3 / 3 )\"\nlast\n body\n",
    )?;
    Ok(())
}

#[tokio::test]
async fn три_выбранные_части_создают_один_результат_при_любом_порядке() -> TestResult {
    for order in [
        ["main.dsl", "part.dsl", "last.dsl"],
        ["last.dsl", "part.dsl", "main.dsl"],
    ] {
        let temp = tempfile::tempdir()?;
        fixture(temp.path())?;
        let storage = support::TestStorage::new(temp.path());
        let manager = JobManager::open(temp.path().join("data"), storage.clone())?;
        let mut selected = drafts(&manager, temp.path(), &order).await;
        for draft in &mut selected {
            draft.auto_start = true;
        }
        let result = manager.create_batch(selected).await?;
        assert_eq!(result.jobs.len(), 1);
        assert_eq!(result.included.len(), 1);
        assert_eq!(result.included[0].main_name, "Main");
        let mut names = result.included[0].dictionary_names.clone();
        names.sort();
        assert_eq!(names, ["last.dsl", "part.dsl"]);
        let done = support::wait(&manager, &result.jobs[0].id, &[JobState::Succeeded]).await;
        assert_eq!(done.result.expect("результат").summary.source_articles, 3);
        assert_eq!(storage.invocations.load(Ordering::SeqCst), 1);
        assert_eq!(manager.list()?.len(), 1);
        manager.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn часть_уже_выбранного_основного_словаря_не_добавляется_повторно() -> TestResult {
    let temp = tempfile::tempdir()?;
    fixture(temp.path())?;
    let manager = JobManager::open(
        temp.path().join("data"),
        support::TestStorage::new(temp.path()),
    )?;
    let main = manager
        .create_batch(drafts(&manager, temp.path(), &["main.dsl"]).await)
        .await?;
    let part = manager
        .create_batch(drafts(&manager, temp.path(), &["part.dsl"]).await)
        .await?;
    assert!(part.jobs.is_empty());
    assert_eq!(part.included[0].main_name, "Main");
    assert_eq!(manager.list()?.len(), 1);
    assert_eq!(manager.list()?[0].id, main.jobs[0].id);
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn добавление_главного_отменяет_отдельную_ожидающую_часть() -> TestResult {
    let temp = tempfile::tempdir()?;
    fixture(temp.path())?;
    let manager = JobManager::open(
        temp.path().join("data"),
        support::TestStorage::new(temp.path()),
    )?;
    let part = manager
        .create(drafts(&manager, temp.path(), &["part.dsl"]).await.remove(0))
        .await?;
    let main = manager
        .create_batch(drafts(&manager, temp.path(), &["main.dsl"]).await)
        .await?;
    assert_eq!(main.jobs.len(), 1);
    assert_eq!(main.included[0].dictionary_names, ["part.dsl"]);
    assert_eq!(
        manager
            .list()?
            .iter()
            .find(|job| job.id == part.id)
            .expect("часть")
            .state,
        JobState::Cancelled
    );
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn одинаковые_имена_в_разных_папках_не_объединяются() -> TestResult {
    let temp = tempfile::tempdir()?;
    fixture(temp.path())?;
    let other = temp.path().join("other");
    std::fs::create_dir(&other)?;
    std::fs::write(other.join("part.dsl"), "other\n body\n")?;
    let manager = JobManager::open(
        temp.path().join("data"),
        support::TestStorage::new(temp.path()),
    )?;
    let mut selected = drafts(&manager, temp.path(), &["main.dsl"]).await;
    selected.extend(drafts(&manager, &other, &["part.dsl"]).await);
    let result = manager.create_batch(selected).await?;
    assert_eq!(result.jobs.len(), 2);
    assert!(result.included.is_empty());
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn параллельный_выбор_той_же_части_не_создаёт_дубликат() -> TestResult {
    let temp = tempfile::tempdir()?;
    fixture(temp.path())?;
    let manager = JobManager::open(
        temp.path().join("data"),
        support::TestStorage::new(temp.path()),
    )?;
    let selected = drafts(&manager, temp.path(), &["part.dsl"]).await;
    let (a, b) = tokio::join!(
        manager.create_batch(selected.clone()),
        manager.create_batch(selected)
    );
    assert_eq!(a?.jobs.len() + b?.jobs.len(), 1);
    assert_eq!(manager.list()?.len(), 1);
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn завершённую_часть_не_отменяет_и_не_удаляет() -> TestResult {
    let temp = tempfile::tempdir()?;
    fixture(temp.path())?;
    let manager = JobManager::open(
        temp.path().join("data"),
        support::TestStorage::new(temp.path()),
    )?;
    let job = manager
        .create(drafts(&manager, temp.path(), &["last.dsl"]).await.remove(0))
        .await?;
    manager.start(&job.id).await?;
    support::wait(&manager, &job.id, &[JobState::Succeeded]).await;
    let directory = manager.result_directory(&job.id)?;
    let main = manager
        .create_batch(drafts(&manager, temp.path(), &["main.dsl"]).await)
        .await?;
    assert_eq!(main.jobs.len(), 1);
    assert!(main.included.is_empty());
    assert_eq!(
        manager
            .list()?
            .iter()
            .find(|item| item.id == job.id)
            .expect("часть")
            .state,
        JobState::Succeeded
    );
    assert!(directory.join("dictionary.mdx").is_file());
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn главный_отменяет_запущенную_часть_в_очереди_конвертации() -> TestResult {
    let temp = tempfile::tempdir()?;
    fixture(temp.path())?;
    std::fs::write(temp.path().join("blocker.dsl"), "blocker\n body\n")?;
    let storage = support::TestStorage::new(temp.path());
    storage.block_publish.store(true, Ordering::SeqCst);
    let manager = JobManager::open(temp.path().join("data"), storage.clone())?;
    let blocker = manager
        .create(
            drafts(&manager, temp.path(), &["blocker.dsl"])
                .await
                .remove(0),
        )
        .await?;
    manager.start(&blocker.id).await?;
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        storage.publish_started.notified(),
    )
    .await?;
    let part = manager
        .create(drafts(&manager, temp.path(), &["part.dsl"]).await.remove(0))
        .await?;
    manager.start(&part.id).await?;
    let main = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        manager.create_batch(drafts(&manager, temp.path(), &["main.dsl"]).await),
    )
    .await??;
    assert_eq!(main.jobs.len(), 1);
    assert_eq!(
        manager
            .list()?
            .iter()
            .find(|job| job.id == part.id)
            .expect("часть")
            .state,
        JobState::Cancelled
    );
    assert_eq!(storage.invocations.load(Ordering::SeqCst), 1);
    storage.block_publish.store(false, Ordering::SeqCst);
    storage.publish_release.notify_one();
    support::wait(&manager, &blocker.id, &[JobState::Succeeded]).await;
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn готовый_результат_не_присваивает_включения_добавленные_позже() -> TestResult {
    let temp = tempfile::tempdir()?;
    fixture(temp.path())?;
    std::fs::write(temp.path().join("main.dsl"), "root\n body\n")?;
    let storage = support::TestStorage::new(temp.path());
    storage.fail_publish.store(true, Ordering::SeqCst);
    let manager = JobManager::open(temp.path().join("data"), storage)?;
    let main = manager
        .create(drafts(&manager, temp.path(), &["main.dsl"]).await.remove(0))
        .await?;
    manager.start(&main.id).await?;
    support::wait(&manager, &main.id, &[JobState::AwaitingInput]).await;
    std::fs::write(
        temp.path().join("main.dsl"),
        "#INCLUDE \"last.dsl\"\nroot\n body\n",
    )?;
    let part = manager
        .create_batch(drafts(&manager, temp.path(), &["last.dsl"]).await)
        .await?;
    assert_eq!(
        part.jobs.len(),
        1,
        "готовые артефакты не содержат добавленную позже часть"
    );
    assert!(part.included.is_empty());
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn готовый_результат_помнит_включения_даже_после_изменения_исходника() -> TestResult {
    let temp = tempfile::tempdir()?;
    fixture(temp.path())?;
    let storage = support::TestStorage::new(temp.path());
    storage.fail_publish.store(true, Ordering::SeqCst);
    let manager = JobManager::open(temp.path().join("data"), storage)?;
    let main = manager
        .create(drafts(&manager, temp.path(), &["main.dsl"]).await.remove(0))
        .await?;
    manager.start(&main.id).await?;
    support::wait(&manager, &main.id, &[JobState::AwaitingInput]).await;
    std::fs::write(temp.path().join("main.dsl"), "root\n body\n")?;
    let part = manager
        .create_batch(drafts(&manager, temp.path(), &["last.dsl"]).await)
        .await?;
    assert!(
        part.jobs.is_empty(),
        "часть уже находится в готовых артефактах"
    );
    assert_eq!(part.included.len(), 1);
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn изменение_параметров_сбрасывает_состав_готовых_артефактов() -> TestResult {
    let temp = tempfile::tempdir()?;
    fixture(temp.path())?;
    let storage = support::TestStorage::new(temp.path());
    storage.fail_publish.store(true, Ordering::SeqCst);
    let manager = JobManager::open(temp.path().join("data"), storage)?;
    let main = manager
        .create(drafts(&manager, temp.path(), &["main.dsl"]).await.remove(0))
        .await?;
    manager.start(&main.id).await?;
    support::wait(&manager, &main.id, &[JobState::AwaitingInput]).await;
    std::fs::write(temp.path().join("main.dsl"), "root\n body\n")?;
    manager
        .update(
            &main.id,
            JobOptionsUpdate {
                options: Some(converter_core::model::ReadOptions {
                    encoding: converter_core::model::EncodingChoice::Manual("utf-8".into()),
                }),
                ..Default::default()
            },
        )
        .await?;
    let part = manager
        .create_batch(drafts(&manager, temp.path(), &["last.dsl"]).await)
        .await?;
    assert_eq!(
        part.jobs.len(),
        1,
        "прежних артефактов уже нет, часть должна обрабатываться"
    );
    assert!(part.included.is_empty());
    manager.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn ручное_описание_не_скрывает_название_основного_в_уведомлении() -> TestResult {
    let temp = tempfile::tempdir()?;
    fixture(temp.path())?;
    let manager = JobManager::open(
        temp.path().join("data"),
        support::TestStorage::new(temp.path()),
    )?;
    let mut draft = drafts(&manager, temp.path(), &["main.dsl"]).await.remove(0);
    draft.metadata_override = Some(converter_core::model::DictionaryMetadata {
        description: "Моё описание".into(),
        ..Default::default()
    });
    manager.create_batch(vec![draft]).await?;
    let part = manager
        .create_batch(drafts(&manager, temp.path(), &["last.dsl"]).await)
        .await?;
    assert!(part.jobs.is_empty());
    assert_eq!(part.included[0].main_name, "Main");
    manager.shutdown().await?;
    Ok(())
}
