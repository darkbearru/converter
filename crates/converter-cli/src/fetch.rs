use converter_core::model::{ConverterError, PreparedFile, Result};
use converter_download::{
    CompletedDownload, DownloadCache, DownloadManager, DownloadPolicy, DownloadRequest,
    TransferControl, TransferOutcome,
};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

fn error(error: converter_download::DownloadError) -> ConverterError {
    ConverterError::new(&error.code, error.message)
}
fn request(url: String, expected_sha256: Option<String>) -> Result<DownloadRequest> {
    let url = url.parse::<url::Url>().map_err(|_| {
        ConverterError::new("invalid_download_url", "Некорректная ссылка HTTP/HTTPS")
    })?;
    // Стабильный непрозрачный ключ позволяет повторять URL-команду после перезапуска.
    let hash = Sha256::new()
        .chain_update(url.as_str())
        .chain_update([0])
        .chain_update(expected_sha256.as_deref().unwrap_or_default())
        .finalize();
    let mut id = [0u8; 16];
    id.copy_from_slice(&hash[..16]);
    Ok(DownloadRequest {
        source_id: uuid::Uuid::from_bytes(id).to_string(),
        url,
        expected_sha256,
    })
}
async fn fetch(
    cache: &DownloadCache,
    request: DownloadRequest,
    control: TransferControl,
) -> Result<CompletedDownload> {
    let manager = DownloadManager::new(cache.clone(), DownloadPolicy::default()).map_err(error)?;
    let previous = Mutex::new((None, Instant::now()));
    match manager
        .fetch(
            request.clone(),
            control,
            Arc::new(move |event| {
                if let Ok(mut last) = previous.lock()
                    && (last.0 != Some(event.state)
                        || last.1.elapsed() >= Duration::from_millis(200))
                {
                    eprintln!(
                        "{}",
                        serde_json::json!({"phase": "download", "progress": event})
                    );
                    *last = (Some(event.state), Instant::now());
                }
            }),
        )
        .await
        .map_err(error)?
    {
        TransferOutcome::Completed(done) => Ok(done),
        TransferOutcome::Paused => Err(ConverterError::new(
            "download_paused",
            format!("Скачивание приостановлено. Источник: {}", request.source_id),
        )),
        TransferOutcome::Cancelled => Err(ConverterError::new(
            "cancelled",
            format!(
                "Скачивание остановлено, сохранённую часть можно продолжить. Источник: {}",
                request.source_id
            ),
        )),
    }
}
pub(crate) async fn run(
    url: Option<String>,
    source_id: Option<String>,
    sha256: Option<String>,
    directory: PathBuf,
    control: TransferControl,
) -> Result<serde_json::Value> {
    let cache = DownloadCache::open(directory).map_err(error)?;
    let request = if let Some(url) = url {
        request(url, sha256)?
    } else {
        cache
            .request(source_id.as_deref().ok_or_else(|| {
                ConverterError::new(
                    "source_required",
                    "Укажите ссылку или идентификатор источника",
                )
            })?)
            .map_err(error)?
    };
    let completed = fetch(&cache, request, control).await?;
    Ok(serde_json::json!({"source": completed}))
}
pub(crate) async fn sources(
    url: String,
    resources: Vec<String>,
    sha256: Option<String>,
    directory: PathBuf,
    control: TransferControl,
) -> Result<Vec<PreparedFile>> {
    let cache = DownloadCache::open(directory).map_err(error)?;
    let mut requests = vec![request(url, sha256)?];
    requests.extend(
        resources
            .into_iter()
            .map(|url| request(url, None))
            .collect::<Result<Vec<_>>>()?,
    );
    let mut files = Vec::new();
    for request in requests {
        let done = fetch(&cache, request, control.clone()).await?;
        let mut prefix = [0u8; 1024];
        let n = std::fs::File::open(&done.path)?.read(&mut prefix)?;
        let text = String::from_utf8_lossy(&prefix[..n])
            .trim_start_matches('\u{feff}')
            .trim_start()
            .to_ascii_lowercase();
        if text.starts_with("<!doctype html") || text.starts_with("<html") {
            return Err(ConverterError::new(
                "download_is_html",
                "Вместо словаря получена HTML-страница. Нужна прямая ссылка на файл",
            ));
        }
        files.push(PreparedFile {
            id: done.source_id,
            display_name: done.display_name,
            path: done.path,
        });
    }
    Ok(files)
}
