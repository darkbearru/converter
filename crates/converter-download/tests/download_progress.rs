use converter_download::*;
use converter_test_support::http::{FixtureServer, Scenario};
use std::sync::{Arc, Mutex};
#[tokio::test]
async fn неизвестный_размер_не_дает_процент() -> std::result::Result<(), Box<dyn std::error::Error>>
{
    assert_eq!(download_percent(1234, None), None);
    assert_eq!(download_percent(1, Some(0)), None);
    assert_eq!(download_percent(50, Some(100)), Some(50.0));
    let server = FixtureServer::spawn(Scenario::UnknownLength, vec![3; 100_000]).await;
    let temp = tempfile::tempdir()?;
    let manager = DownloadManager::new(
        DownloadCache::open(temp.path().to_owned())?,
        DownloadPolicy::default(),
    )?;
    let events = Arc::new(Mutex::new(Vec::new()));
    let output = events.clone();
    let req = DownloadRequest {
        source_id: uuid::Uuid::new_v4().to_string(),
        url: server.url().parse()?,
        expected_sha256: None,
    };
    manager
        .fetch(
            req,
            TransferControl::default(),
            Arc::new(move |event| output.lock().expect("events").push(event)),
        )
        .await?;
    let events = events.lock().expect("events");
    assert!(
        events
            .iter()
            .filter(|p| p.state == TransferState::Downloading)
            .all(|p| p.total_bytes.is_none())
    );
    assert_eq!(
        events.last().expect("completed").state,
        TransferState::Completed
    );
    assert_eq!(events.last().expect("bytes").received_bytes, 100_000);
    assert!(events.iter().all(|p| p.confirmed_bytes <= p.received_bytes));
    Ok(())
}
