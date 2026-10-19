mod support;
use converter_app::{storage::PlatformSelection, *};
use std::{fs::File, io::Write};
use zip::{AesMode, ZipWriter, write::SimpleFileOptions};
#[tokio::test]
async fn пароль_можно_повторить_он_не_попадает_в_историю() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("locked.zip");
    let mut writer = ZipWriter::new(File::create(&source)?);
    writer.start_file(
        "main.dsl",
        SimpleFileOptions::default().with_aes_encryption(AesMode::Aes256, "secret-value"),
    )?;
    writer.write_all(b"word\n body\n")?;
    writer.finish()?;
    let data = temp.path().join("data");
    let manager = JobManager::open(data.clone(), support::TestStorage::new(temp.path()))?;
    let import = manager
        .import(PlatformSelection::LocalPaths(vec![source]))
        .await?
        .remove(0);
    let job = manager
        .create(support::draft(SourceInput::Local {
            import_id: import.import_id.clone(),
        }))
        .await?;
    manager.start(&job.id).await?;
    let waiting = support::wait(&manager, &job.id, &[JobState::AwaitingInput]).await;
    assert_eq!(
        waiting.error.as_ref().map(|e| e.code.as_str()),
        Some("archive_password_required")
    );
    assert!(
        manager
            .provide_archive_password(&job.id, "another-archive", "secret-value".into())
            .await
            .is_err()
    );
    manager
        .provide_archive_password(&job.id, &import.import_id, "wrong".into())
        .await?;
    let invalid = support::wait(&manager, &job.id, &[JobState::AwaitingInput]).await;
    assert_eq!(
        invalid.error.as_ref().map(|e| e.code.as_str()),
        Some("archive_password_invalid")
    );
    manager
        .provide_archive_password(&job.id, &import.import_id, "secret-value".into())
        .await?;
    support::wait(&manager, &job.id, &[JobState::Succeeded]).await;
    manager.shutdown().await?;
    let history = std::fs::read_to_string(data.join("jobs").join(format!("{}.json", job.id)))?;
    assert!(!history.contains("secret-value") && !history.contains("wrong"));
    Ok(())
}

#[tokio::test]
async fn скачанный_zip_после_ввода_пароля_использует_готовый_кэш()
-> Result<(), Box<dyn std::error::Error>> {
    use converter_test_support::http::{FixtureServer, Scenario};
    let temp = tempfile::tempdir()?;
    let mut bytes = std::io::Cursor::new(Vec::new());
    {
        let mut writer = ZipWriter::new(&mut bytes);
        writer.start_file(
            "main.dsl",
            SimpleFileOptions::default().with_aes_encryption(AesMode::Aes256, "remote-secret"),
        )?;
        writer.write_all(b"word\n body\n")?;
        writer.finish()?;
    }
    let server = FixtureServer::spawn(Scenario::StableRange, bytes.into_inner()).await;
    let manager = JobManager::open(
        temp.path().join("data"),
        support::TestStorage::new(temp.path()),
    )?;
    let mut draft = support::draft(support::remote(server.url()));
    draft.prepare_only = true;
    let job = manager.create(draft).await?;
    manager.start(&job.id).await?;
    support::wait(&manager, &job.id, &[JobState::Ready]).await;
    manager.start(&job.id).await?;
    let waiting = support::wait(&manager, &job.id, &[JobState::AwaitingInput]).await;
    let archive_id = waiting
        .error
        .as_ref()
        .and_then(|e| e.message_args["archive_id"].as_str())
        .ok_or("archive id")?;
    let before = server.requests().len();
    manager
        .provide_archive_password(&job.id, archive_id, "remote-secret".into())
        .await?;
    support::wait(&manager, &job.id, &[JobState::Succeeded]).await;
    assert_eq!(server.requests().len(), before);
    manager.shutdown().await?;
    Ok(())
}
