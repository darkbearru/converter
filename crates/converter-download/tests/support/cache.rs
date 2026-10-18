use converter_download::{Checkpoint, ChunkHash, DownloadRequest};
use sha2::{Digest, Sha256};
pub fn request() -> DownloadRequest {
    DownloadRequest {
        source_id: uuid::Uuid::new_v4().to_string(),
        url: "https://example.org/dict?secret-token"
            .parse()
            .expect("URL"),
        expected_sha256: None,
    }
}
pub fn checkpoint(bytes: &[u8]) -> Checkpoint {
    Checkpoint {
        confirmed_bytes: bytes.len() as u64,
        chunk_hashes: vec![ChunkHash {
            bytes: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
        }],
    }
}
