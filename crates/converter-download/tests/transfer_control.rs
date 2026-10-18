use converter_download::*;
use converter_test_support::http::{FixtureServer, Scenario};
use sha2::{Digest, Sha256};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
fn request(url: String) -> DownloadRequest {
    DownloadRequest {
        source_id: uuid::Uuid::new_v4().to_string(),
        url: url.parse().expect("URL"),
        expected_sha256: None,
    }
}
fn policy() -> DownloadPolicy {
    DownloadPolicy {
        retry_delays: vec![Duration::ZERO; 3],
        ..Default::default()
    }
}
fn done(outcome: TransferOutcome) -> CompletedDownload {
    match outcome {
        TransferOutcome::Completed(done) => done,
        _ => panic!("expected completed"),
    }
}

#[tokio::test]
async fn пауза_сохраняет_порцию_и_сбрасывает_скорость()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let bytes = vec![42; 3 * 1_048_576];
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let server = FixtureServer::spawn(Scenario::HoldAt(1_048_576), bytes).await;
    let temp = tempfile::tempdir()?;
    let cache = DownloadCache::open(temp.path().to_owned())?;
    let manager = DownloadManager::new(cache.clone(), policy())?;
    let req = request(server.url());
    let control = TransferControl::default();
    let signal = control.clone();
    let events = Arc::new(Mutex::new(Vec::new()));
    let out = events.clone();
    let outcome = manager
        .fetch(
            req.clone(),
            control,
            Arc::new(move |event| {
                if event.received_bytes >= 1_048_576 && event.state == TransferState::Downloading {
                    signal.pause();
                }
                out.lock().expect("events").push(event);
            }),
        )
        .await?;
    assert!(matches!(outcome, TransferOutcome::Paused));
    assert_eq!(
        cache.verify_prefix(&req.source_id)?.confirmed_bytes,
        1_048_576
    );
    assert!(cache.completed(&req.source_id)?.is_none());
    assert!(
        events
            .lock()
            .expect("events")
            .last()
            .expect("pause")
            .bytes_per_second
            .is_none()
    );
    server.release();
    let reopened = DownloadManager::new(DownloadCache::open(temp.path().to_owned())?, policy())?;
    let completed = done(
        reopened
            .fetch(req.clone(), TransferControl::default(), Arc::new(|_| {}))
            .await?,
    );
    assert_eq!(completed.sha256, hash);
    assert_eq!(server.requests()[1].range_start, Some(1_048_576));
    let count = server.requests().len();
    assert_eq!(
        done(
            reopened
                .fetch(req, TransferControl::default(), Arc::new(|_| {}))
                .await?
        )
        .sha256,
        hash
    );
    assert_eq!(server.requests().len(), count);
    Ok(())
}

#[tokio::test]
async fn проверяет_готовность_перед_completed()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let server = FixtureServer::spawn(Scenario::StableRange, vec![8; 200_000]).await;
    let temp = tempfile::tempdir()?;
    let cache = DownloadCache::open(temp.path().to_owned())?;
    let manager = DownloadManager::new(cache.clone(), policy())?;
    let mut req = request(server.url());
    req.expected_sha256 = Some("0".repeat(64));
    let error = manager
        .fetch(req.clone(), TransferControl::default(), Arc::new(|_| {}))
        .await
        .expect_err("digest");
    assert_eq!(error.code, "download_digest_mismatch");
    assert!(cache.completed(&req.source_id)?.is_none());
    let req = request(server.url());
    let control = TransferControl::default();
    let signal = control.clone();
    let outcome = manager
        .fetch(
            req.clone(),
            control,
            Arc::new(move |event| {
                if event.state == TransferState::Verifying {
                    signal.pause();
                }
            }),
        )
        .await?;
    assert!(matches!(outcome, TransferOutcome::Paused));
    assert!(cache.completed(&req.source_id)?.is_none());
    let count = server.requests().len();
    done(
        manager
            .fetch(req, TransferControl::default(), Arc::new(|_| {}))
            .await?,
    );
    assert_eq!(server.requests().len(), count);
    Ok(())
}

#[tokio::test]
async fn повторы_ограничены_и_ошибки_не_раскрывают_url()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    for status in [403, 503, 429] {
        let server = FixtureServer::spawn(
            Scenario::Status {
                status,
                retry_after: Some("0".into()),
            },
            vec![],
        )
        .await;
        let temp = tempfile::tempdir()?;
        let manager = DownloadManager::new(DownloadCache::open(temp.path().to_owned())?, policy())?;
        let error = manager
            .fetch(
                request(format!("{}?secret-token", server.url())),
                TransferControl::default(),
                Arc::new(|_| {}),
            )
            .await
            .expect_err("HTTP");
        assert_eq!(error.code, format!("http_{status}"));
        assert!(!error.to_string().contains("secret-token"));
        assert_eq!(server.requests().len(), if status == 403 { 1 } else { 4 });
    }
    let server = FixtureServer::spawn(
        Scenario::Status {
            status: 429,
            retry_after: Some("301".into()),
        },
        vec![],
    )
    .await;
    let temp = tempfile::tempdir()?;
    let manager = DownloadManager::new(DownloadCache::open(temp.path().to_owned())?, policy())?;
    assert!(matches!(
        manager
            .fetch(
                request(server.url()),
                TransferControl::default(),
                Arc::new(|_| {})
            )
            .await?,
        TransferOutcome::Paused
    ));
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn ожидание_повтора_прерывается_управлением() {
    let control = TransferControl::default();
    let signal = control.clone();
    let wait = tokio::spawn(async move { signal.wait_retry(Duration::from_secs(300)).await });
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(2)).await;
    assert!(!wait.is_finished());
    control.cancel();
    assert!(!wait.await.expect("wait"));
}

#[test]
fn учитывает_http_дату_retry_after() {
    let now = std::time::UNIX_EPOCH + Duration::from_secs(1_000_000);
    assert_eq!(
        converter_download::policy::retry_after("Thu, 01 Jan 1970 00:00:00 GMT", now),
        Some(Duration::ZERO)
    );
    assert_eq!(
        converter_download::policy::retry_after("15", now),
        Some(Duration::from_secs(15))
    );
    assert_eq!(converter_download::policy::retry_after("wrong", now), None);
}

#[tokio::test]
async fn повреждение_перед_sha_и_отмена_не_создают_completed()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let server = FixtureServer::spawn(Scenario::StableRange, vec![6; 200_000]).await;
    let temp = tempfile::tempdir()?;
    let cache = DownloadCache::open(temp.path().to_owned())?;
    let manager = DownloadManager::new(cache.clone(), policy())?;
    let req = request(server.url());
    let control = TransferControl::default();
    let signal = control.clone();
    assert!(matches!(
        manager
            .fetch(
                req.clone(),
                control,
                Arc::new(move |p| if p.state == TransferState::Verifying {
                    signal.pause();
                })
            )
            .await?,
        TransferOutcome::Paused
    ));
    std::fs::write(cache.part_path(&req.source_id)?, vec![5; 200_000])?;
    assert_eq!(
        manager
            .fetch(req, TransferControl::default(), Arc::new(|_| {}))
            .await
            .expect_err("corruption")
            .code,
        "checkpoint_hash_mismatch"
    );
    let req = request(server.url());
    let control = TransferControl::default();
    let signal = control.clone();
    assert!(matches!(
        manager
            .fetch(
                req.clone(),
                control,
                Arc::new(move |p| {
                    if p.received_bytes >= 65536 && p.state == TransferState::Downloading {
                        signal.cancel();
                    }
                })
            )
            .await?,
        TransferOutcome::Cancelled
    ));
    assert!(cache.completed(&req.source_id)?.is_none());
    assert!(cache.verify_prefix(&req.source_id)?.confirmed_bytes >= 65536);
    Ok(())
}

#[tokio::test]
async fn обрыв_докачивается_а_превышение_лимита_не_готово()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let bytes = vec![9; 2_097_152];
    let expected = format!("{:x}", Sha256::digest(&bytes));
    let server = FixtureServer::spawn(Scenario::DisconnectAt(1_048_576), bytes).await;
    let temp = tempfile::tempdir()?;
    let cache = DownloadCache::open(temp.path().to_owned())?;
    let manager = DownloadManager::new(cache, policy())?;
    let completed = done(
        manager
            .fetch(
                request(server.url()),
                TransferControl::default(),
                Arc::new(|_| {}),
            )
            .await?,
    );
    assert_eq!(completed.sha256, expected);
    assert_eq!(server.requests()[1].range_start, Some(1_048_576));
    let server = FixtureServer::spawn(Scenario::UnknownLength, vec![9; 200_000]).await;
    let temp = tempfile::tempdir()?;
    let cache = DownloadCache::open(temp.path().to_owned())?;
    let req = request(server.url());
    let manager = DownloadManager::new(
        cache.clone(),
        DownloadPolicy {
            max_bytes: 100_000,
            ..policy()
        },
    )?;
    assert_eq!(
        manager
            .fetch(req.clone(), TransferControl::default(), Arc::new(|_| {}))
            .await
            .expect_err("limit")
            .code,
        "download_size_limit"
    );
    assert!(cache.completed(&req.source_id)?.is_none());
    Ok(())
}

#[tokio::test]
async fn полный_ответ_новый_etag_и_416_перезапускают_файл()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    for (scenario, length, changed) in [
        (Scenario::IgnoreRange, 2_097_152, false),
        (Scenario::ChangeEtag, 2_097_152, true),
        (Scenario::WeakEtag, 2_097_152, false),
        (Scenario::NoEtag, 2_097_152, false),
        (Scenario::StableRange, 1_048_576, false),
    ] {
        let server = FixtureServer::spawn(scenario, vec![3; length]).await;
        let temp = tempfile::tempdir()?;
        let cache = DownloadCache::open(temp.path().to_owned())?;
        let manager = DownloadManager::new(cache, policy())?;
        let req = request(server.url());
        let control = TransferControl::default();
        let signal = control.clone();
        assert!(matches!(
            manager
                .fetch(
                    req.clone(),
                    control,
                    Arc::new(move |p| {
                        if p.received_bytes >= 1_048_576 && p.state == TransferState::Downloading {
                            signal.pause();
                        }
                    })
                )
                .await?,
            TransferOutcome::Paused
        ));
        let result = done(
            manager
                .fetch(req, TransferControl::default(), Arc::new(|_| {}))
                .await?,
        );
        assert_eq!(
            result.sha256,
            format!(
                "{:x}",
                Sha256::digest(vec![if changed { 4 } else { 3 }; length])
            )
        );
    }
    Ok(())
}
