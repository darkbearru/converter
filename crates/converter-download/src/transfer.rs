use crate::{
    progress::Progress,
    resume::{ResponseAction, content_range, plan_response, strong_etag},
    *,
};
use sha2::{Digest, Sha256};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::Duration,
};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Notify,
};

#[derive(Clone, Default)]
pub struct TransferControl {
    signal: Arc<AtomicU8>,
    notify: Arc<Notify>,
}
impl TransferControl {
    pub fn pause(&self) {
        let _ = self
            .signal
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst);
        self.notify.notify_waiters();
    }
    pub fn cancel(&self) {
        self.signal.store(2, Ordering::SeqCst);
        self.notify.notify_waiters();
    }
    pub fn state(&self) -> TransferState {
        match self.signal.load(Ordering::SeqCst) {
            1 => TransferState::Paused,
            2 => TransferState::Cancelled,
            _ => TransferState::Pending,
        }
    }
    async fn stopped(&self) {
        loop {
            let future = self.notify.notified();
            tokio::pin!(future);
            future.as_mut().enable();
            if self.state() != TransferState::Pending {
                return;
            }
            future.await;
        }
    }
    pub async fn wait_retry(&self, delay: Duration) -> bool {
        tokio::select! { biased; _ = self.stopped() => false, _ = tokio::time::sleep(delay) => true }
    }
}
#[derive(Clone)]
pub struct DownloadManager {
    cache: DownloadCache,
    policy: DownloadPolicy,
}
impl DownloadManager {
    pub fn new(cache: DownloadCache, policy: DownloadPolicy) -> Result<Self> {
        policy::validate(&policy)?;
        Ok(Self { cache, policy })
    }
    pub async fn fetch(
        &self,
        request: DownloadRequest,
        control: TransferControl,
        sink: Arc<dyn Fn(DownloadProgress) + Send + Sync>,
    ) -> Result<TransferOutcome> {
        let mut progress = Progress::new(request.source_id.clone(), sink);
        let result = self.fetch_inner(&request, &control, &mut progress).await;
        if result.is_err() {
            progress.emit(TransferState::Failed);
        }
        result
    }
    async fn fetch_inner(
        &self,
        request: &DownloadRequest,
        control: &TransferControl,
        progress: &mut Progress,
    ) -> Result<TransferOutcome> {
        let id = &request.source_id;
        let _guard = self.cache.acquire(id)?;
        self.cache.begin(request)?;
        let manifest = self.cache.required(id)?;
        progress.reset(manifest.checkpoint.confirmed_bytes);
        progress.value.total_bytes = manifest.total_bytes;
        progress.emit(TransferState::Pending);
        if manifest.completed.is_some() || manifest.transfer_finished {
            return self.finish(request, control, progress).await;
        }
        let mut retry = 0;
        let mut restarts = 0;
        loop {
            if let Some(outcome) = stopped(control, progress) {
                return Ok(outcome);
            }
            let point = self.cache.verify_prefix(id)?;
            progress.reset(point.confirmed_bytes);
            progress.value.total_bytes = point.total_bytes;
            let opened = tokio::select! { biased; _ = control.stopped() => return Ok(stopped(control, progress).expect("control stopped")), response = http::open(request, Some(&point), &self.policy) => response };
            let opened = match opened {
                Ok(response) => response,
                Err(error) if error.code == "download_network" => {
                    if self.retry(retry, None, control, progress).await {
                        retry += 1;
                        continue;
                    }
                    if let Some(outcome) = stopped(control, progress) {
                        return Ok(outcome);
                    }
                    return Err(error);
                }
                Err(error) => return Err(error),
            };
            let action = plan_response(Some(&point), &opened.metadata)?;
            match action {
                ResponseAction::Reject { code } => {
                    if policy::retryable(opened.metadata.status) {
                        if opened
                            .metadata
                            .retry_after
                            .is_some_and(|delay| delay > Duration::from_secs(300))
                        {
                            progress.value.restart_reason = Some(
                                "Сервер просит подождать больше пяти минут; продолжите позже"
                                    .into(),
                            );
                            progress.emit(TransferState::Paused);
                            return Ok(TransferOutcome::Paused);
                        }
                        if self
                            .retry(retry, opened.metadata.retry_after, control, progress)
                            .await
                        {
                            retry += 1;
                            continue;
                        }
                        if let Some(outcome) = stopped(control, progress) {
                            return Ok(outcome);
                        }
                    }
                    return Err(DownloadError::new(
                        &code,
                        &format!("Сервер отклонил загрузку (HTTP {})", opened.metadata.status),
                    ));
                }
                ResponseAction::Restart { reason } => {
                    if restarts >= 2 {
                        return Err(DownloadError::new(
                            "unsafe_resume",
                            "Сервер не предоставил безопасный ответ",
                        ));
                    }
                    // Старые байты сохраняются до принятия новых headers; запрос без Range.
                    let fresh = tokio::select! { biased; _ = control.stopped() => return Ok(stopped(control, progress).expect("control stopped")), response = http::open(request, None, &self.policy) => response? };
                    if plan_response(None, &fresh.metadata)? != ResponseAction::StartFresh {
                        return Err(DownloadError::new(
                            "unsafe_resume",
                            "Сервер не предоставил полный файл",
                        ));
                    }
                    progress.value.restart_reason = Some(reason);
                    restarts += 1;
                    self.accept_fresh(id, &fresh.metadata, progress).await?;
                    match self.transfer(id, fresh, control, progress).await {
                        Ok(true) => return self.finish(request, control, progress).await,
                        Ok(false) => {
                            return Ok(stopped(control, progress).expect("control stopped"));
                        }
                        Err(e) if e.code == "download_network" => {
                            if self.retry(retry, None, control, progress).await {
                                retry += 1;
                                continue;
                            }
                            if let Some(outcome) = stopped(control, progress) {
                                return Ok(outcome);
                            }
                            return Err(e);
                        }
                        Err(e) => return Err(e),
                    }
                }
                ResponseAction::StartFresh => {
                    if point.confirmed_bytes > 0 {
                        progress.value.restart_reason =
                            Some("Сервер вернул полный файл; загрузка начнётся заново".into());
                    }
                    self.accept_fresh(id, &opened.metadata, progress).await?;
                }
                ResponseAction::Append { .. } => {
                    progress.value.total_bytes = opened
                        .metadata
                        .content_range
                        .as_deref()
                        .and_then(content_range)
                        .map(|(_, _, total)| total);
                }
            }
            match self.transfer(id, opened, control, progress).await {
                Ok(true) => return self.finish(request, control, progress).await,
                Ok(false) => return Ok(stopped(control, progress).expect("control stopped")),
                Err(error) if error.code == "download_network" => {
                    if self.retry(retry, None, control, progress).await {
                        retry += 1;
                        continue;
                    }
                    if let Some(outcome) = stopped(control, progress) {
                        return Ok(outcome);
                    }
                    return Err(error);
                }
                Err(error) => return Err(error),
            }
        }
    }
    async fn retry(
        &self,
        attempt: usize,
        after: Option<Duration>,
        control: &TransferControl,
        progress: &mut Progress,
    ) -> bool {
        let Some(delay) = self.policy.retry_delays.get(attempt) else {
            return false;
        };
        progress.emit(TransferState::RetryWait);
        control
            .wait_retry((*delay).max(after.unwrap_or_default()))
            .await
    }
    async fn accept_fresh(
        &self,
        id: &str,
        metadata: &resume::ResponseMetadata,
        progress: &mut Progress,
    ) -> Result<()> {
        let mut manifest = self.cache.required(id)?;
        manifest.checkpoint = Checkpoint::default();
        manifest.completed = None;
        manifest.transfer_finished = false;
        manifest.effective_url = Some(metadata.effective_url.clone());
        manifest.validator = metadata.etag.clone().filter(|tag| strong_etag(tag));
        manifest.total_bytes = metadata.content_length;
        manifest.display_name = metadata.display_name.clone();
        manifest.generation += 1;
        self.cache.save(&manifest)?;
        // Сначала долговременный нулевой offset, затем обнуление файла.
        OpenOptions::new()
            .write(true)
            .open(self.cache.part_path(id)?)
            .await?
            .set_len(0)
            .await?;
        progress.reset(0);
        progress.value.total_bytes = metadata.content_length;
        Ok(())
    }
    async fn transfer(
        &self,
        id: &str,
        mut opened: http::OpenResponse,
        control: &TransferControl,
        progress: &mut Progress,
    ) -> Result<bool> {
        let mut checkpoint = self.cache.required(id)?.checkpoint;
        let mut file = OpenOptions::new()
            .append(true)
            .open(self.cache.part_path(id)?)
            .await?;
        let mut chunk_hash = Sha256::new();
        let mut chunk_bytes = 0;
        let initial = progress.value.received_bytes;
        let expected = opened.metadata.content_length;
        progress.emit(TransferState::Downloading);
        loop {
            if control.state() != TransferState::Pending {
                self.checkpoint(
                    id,
                    &mut file,
                    &mut checkpoint,
                    &mut chunk_hash,
                    &mut chunk_bytes,
                    progress,
                )
                .await?;
                return Ok(false);
            }
            let bytes = tokio::select! { biased; _ = control.stopped() => { self.checkpoint(id, &mut file, &mut checkpoint, &mut chunk_hash, &mut chunk_bytes, progress).await?; return Ok(false); }, result = opened.response.chunk() => result.map_err(|_| http::network_error())? };
            let Some(bytes) = bytes else {
                break;
            };
            let mut pending = bytes.as_ref();
            while !pending.is_empty() {
                if control.state() != TransferState::Pending {
                    self.checkpoint(
                        id,
                        &mut file,
                        &mut checkpoint,
                        &mut chunk_hash,
                        &mut chunk_bytes,
                        progress,
                    )
                    .await?;
                    return Ok(false);
                }
                let n = pending
                    .len()
                    .min(65536)
                    .min((self.policy.checkpoint_bytes - chunk_bytes) as usize);
                let piece = &pending[..n];
                if progress
                    .value
                    .received_bytes
                    .checked_add(n as u64)
                    .is_none_or(|total| total > self.policy.max_bytes)
                {
                    return Err(DownloadError::new(
                        "download_size_limit",
                        "Размер источника превышает допустимый",
                    ));
                }
                file.write_all(piece).await?;
                chunk_hash.update(piece);
                chunk_bytes += n as u64;
                progress.value.received_bytes += n as u64;
                pending = &pending[n..];
                if chunk_bytes == self.policy.checkpoint_bytes {
                    self.checkpoint(
                        id,
                        &mut file,
                        &mut checkpoint,
                        &mut chunk_hash,
                        &mut chunk_bytes,
                        progress,
                    )
                    .await?;
                }
                progress.emit(TransferState::Downloading);
            }
        }
        if expected.is_some_and(|length| length != progress.value.received_bytes - initial)
            || progress
                .value
                .total_bytes
                .is_some_and(|total| total != progress.value.received_bytes)
        {
            return Err(http::network_error());
        }
        self.checkpoint(
            id,
            &mut file,
            &mut checkpoint,
            &mut chunk_hash,
            &mut chunk_bytes,
            progress,
        )
        .await?;
        drop(file);
        let mut manifest = self.cache.required(id)?;
        manifest.transfer_finished = true;
        manifest.generation += 1;
        self.cache.save(&manifest)?;
        Ok(true)
    }
    async fn checkpoint(
        &self,
        id: &str,
        file: &mut File,
        checkpoint: &mut Checkpoint,
        hash: &mut Sha256,
        bytes: &mut u64,
        progress: &mut Progress,
    ) -> Result<()> {
        file.flush().await?;
        if *bytes > 0 {
            checkpoint.chunk_hashes.push(ChunkHash {
                bytes: *bytes,
                sha256: format!("{:x}", std::mem::take(hash).finalize()),
            });
            checkpoint.confirmed_bytes += *bytes;
            *bytes = 0;
        }
        self.cache.commit_checkpoint(id, checkpoint.clone())?;
        progress.value.confirmed_bytes = checkpoint.confirmed_bytes;
        Ok(())
    }
    async fn finish(
        &self,
        request: &DownloadRequest,
        control: &TransferControl,
        progress: &mut Progress,
    ) -> Result<TransferOutcome> {
        progress.emit(TransferState::Verifying);
        let path = self.cache.part_path(&request.source_id)?;
        if !tokio::fs::symlink_metadata(&path).await?.is_file() {
            return Err(DownloadError::new(
                "invalid_cache_file",
                "В кэше ожидается обычный файл",
            ));
        }
        let mut manifest = self.cache.required(&request.source_id)?;
        let mut file = File::open(&path).await?;
        let mut hash = Sha256::new();
        let mut bytes = 0u64;
        let mut buffer = vec![0u8; 65536];
        for chunk in &manifest.checkpoint.chunk_hashes {
            let mut remaining = chunk.bytes;
            let mut chunk_hash = Sha256::new();
            while remaining > 0 {
                if let Some(outcome) = stopped(control, progress) {
                    return Ok(outcome);
                }
                let wanted = buffer.len().min(remaining as usize);
                let n = file.read(&mut buffer[..wanted]).await?;
                if n == 0 {
                    return Err(DownloadError::new(
                        "checkpoint_hash_mismatch",
                        "Сохранённая часть повреждена",
                    ));
                }
                hash.update(&buffer[..n]);
                chunk_hash.update(&buffer[..n]);
                bytes += n as u64;
                remaining -= n as u64;
                tokio::task::yield_now().await;
            }
            if format!("{:x}", chunk_hash.finalize()) != chunk.sha256 {
                return Err(DownloadError::new(
                    "checkpoint_hash_mismatch",
                    "Сохранённая часть повреждена",
                ));
            }
            progress.emit(TransferState::Verifying);
        }
        if file.read(&mut buffer[..1]).await? != 0 {
            return Err(DownloadError::new(
                "download_length_mismatch",
                "Размер локального файла изменился",
            ));
        }
        drop(file);
        if let Some(outcome) = stopped(control, progress) {
            return Ok(outcome);
        }
        let hash = format!("{:x}", hash.finalize());
        if bytes != manifest.checkpoint.confirmed_bytes
            || manifest.total_bytes.is_some_and(|total| total != bytes)
        {
            return Err(DownloadError::new(
                "download_length_mismatch",
                "Размер локального файла изменился",
            ));
        }
        if request
            .expected_sha256
            .as_ref()
            .is_some_and(|expected| !hash.eq_ignore_ascii_case(expected))
        {
            return Err(DownloadError::new(
                "download_digest_mismatch",
                "SHA-256 не совпадает с указанным эталоном",
            ));
        }
        if manifest
            .completed
            .as_ref()
            .is_some_and(|done| done.sha256 != hash)
        {
            return Err(DownloadError::new(
                "completed_hash_mismatch",
                "Готовая локальная копия повреждена",
            ));
        }
        let completed = CompletedDownload {
            source_id: request.source_id.clone(),
            path,
            display_name: manifest.display_name.clone(),
            bytes,
            sha256: hash,
        };
        manifest.completed = Some(completed.clone());
        manifest.generation += 1;
        self.cache.save(&manifest)?;
        progress.value.received_bytes = bytes;
        progress.value.confirmed_bytes = bytes;
        progress.emit(TransferState::Completed);
        Ok(TransferOutcome::Completed(completed))
    }
}
fn stopped(control: &TransferControl, progress: &mut Progress) -> Option<TransferOutcome> {
    match control.state() {
        TransferState::Paused => {
            progress.emit(TransferState::Paused);
            Some(TransferOutcome::Paused)
        }
        TransferState::Cancelled => {
            progress.emit(TransferState::Cancelled);
            Some(TransferOutcome::Cancelled)
        }
        _ => None,
    }
}
