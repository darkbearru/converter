use converter_download::{Checkpoint, ChunkHash, CommitPoint, DownloadCache, DownloadRequest};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
};

fn request() -> DownloadRequest {
    DownloadRequest {
        source_id: uuid::Uuid::new_v4().to_string(),
        url: "https://example.org/dict?secret-token"
            .parse()
            .expect("URL"),
        expected_sha256: None,
    }
}
fn checkpoint(bytes: &[u8]) -> Checkpoint {
    Checkpoint {
        confirmed_bytes: bytes.len() as u64,
        chunk_hashes: vec![ChunkHash {
            bytes: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
        }],
    }
}

#[test]
fn восстанавливает_только_подтвержденный_префикс() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let cache = DownloadCache::open(temp.path().to_owned())?;
    let req = request();
    cache.begin(&req)?;
    let part = cache.part_path(&req.source_id)?;
    let bytes = vec![42; 1_048_576];
    fs::write(&part, &bytes)?;
    cache.commit_checkpoint(&req.source_id, checkpoint(&bytes))?;
    OpenOptions::new()
        .append(true)
        .open(&part)?
        .write_all(b"unconfirmed")?;
    let cache = DownloadCache::open(temp.path().to_owned())?;
    assert_eq!(
        cache.verify_prefix(&req.source_id)?.confirmed_bytes,
        1_048_576
    );
    assert_eq!(fs::metadata(&part)?.len(), 1_048_576);
    assert!(cache.completed(&req.source_id)?.is_none());
    fs::write(&part, vec![41; 1_048_576])?;
    assert_eq!(
        cache
            .verify_prefix(&req.source_id)
            .err()
            .expect("corruption")
            .code,
        "checkpoint_hash_mismatch"
    );
    Ok(())
}

#[test]
fn сбой_контрольной_точки_не_опережает_байты() -> Result<(), Box<dyn std::error::Error>> {
    for point in [
        CommitPoint::BeforeSync,
        CommitPoint::AfterSync,
        CommitPoint::BeforeManifest,
        CommitPoint::AfterManifest,
    ] {
        let temp = tempfile::tempdir()?;
        let cache = DownloadCache::open(temp.path().to_owned())?;
        let req = request();
        cache.begin(&req)?;
        fs::write(cache.part_path(&req.source_id)?, b"abcd")?;
        let result = cache.commit_checkpoint_with_hook(&req.source_id, checkpoint(b"abcd"), |at| {
            if at == point {
                Err(io::Error::from(io::ErrorKind::StorageFull))
            } else {
                Ok(())
            }
        });
        assert!(result.is_err());
        let reopened = DownloadCache::open(temp.path().to_owned())?;
        let resume = reopened.verify_prefix(&req.source_id)?;
        assert_eq!(
            resume.confirmed_bytes,
            if point == CommitPoint::AfterManifest {
                4
            } else {
                0
            }
        );
        assert_eq!(
            fs::metadata(cache.part_path(&req.source_id)?)?.len(),
            resume.confirmed_bytes
        );
    }
    Ok(())
}

#[test]
fn поколения_версия_и_блокировка_кэша() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let cache = DownloadCache::open(temp.path().to_owned())?;
    let req = request();
    cache.begin(&req)?;
    let guard = cache.acquire(&req.source_id)?;
    assert!(cache.acquire(&req.source_id).is_err());
    drop(guard);
    assert!(cache.acquire(&req.source_id).is_ok());
    assert!(cache.part_path("../../outside").is_err());
    fs::write(cache.part_path(&req.source_id)?, b"abcd")?;
    cache.commit_checkpoint(&req.source_id, checkpoint(b"abcd"))?;
    // Сломанное последнее поколение оставляет предыдущую целую запись.
    fs::write(cache.manifest_path(&req.source_id, 0)?, b"{incomplete")?;
    assert_eq!(cache.verify_prefix(&req.source_id)?.confirmed_bytes, 0);
    let other = request();
    cache.begin(&other)?;
    cache.remove(&req.source_id)?;
    assert!(cache.load(&other.source_id)?.is_some());
    let path = cache.manifest_path(&other.source_id, 1)?;
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
    value["version"] = 999.into();
    fs::write(path, serde_json::to_vec(&value)?)?;
    assert_eq!(
        cache.load(&other.source_id).err().expect("version").code,
        "manifest_version_mismatch"
    );
    Ok(())
}
