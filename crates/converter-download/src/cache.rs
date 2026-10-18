use crate::{
    Checkpoint, CommitPoint, CompletedDownload, DownloadError, DownloadManifest, DownloadRequest,
    Result, ResumePoint,
};
use atomicwrites::{AtomicFile, OverwriteBehavior::AllowOverwrite};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct DownloadCache {
    root: PathBuf,
}
pub struct SourceGuard {
    _file: File,
}
fn regular(path: &Path) -> Result<()> {
    if let Ok(meta) = fs::symlink_metadata(path)
        && !meta.is_file()
    {
        return Err(DownloadError::new(
            "invalid_cache_file",
            "В кэше ожидается обычный файл",
        ));
    }
    Ok(())
}
impl DownloadCache {
    pub fn open(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        }
        Ok(Self {
            root: root.canonicalize()?,
        })
    }
    fn path(&self, id: &str, suffix: &str) -> Result<PathBuf> {
        if uuid::Uuid::parse_str(id).is_err()
            || id.len() != 36
            || !id.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-')
        {
            return Err(DownloadError::new(
                "invalid_source_id",
                "Некорректный идентификатор источника",
            ));
        }
        Ok(self.root.join(format!("{id}.{suffix}")))
    }
    pub fn part_path(&self, id: &str) -> Result<PathBuf> {
        self.path(id, "part")
    }
    pub fn manifest_path(&self, id: &str, slot: u8) -> Result<PathBuf> {
        if slot > 1 {
            return Err(DownloadError::new(
                "invalid_manifest_slot",
                "Некорректное поколение манифеста",
            ));
        }
        self.path(id, &format!("manifest-{slot}.json"))
    }
    pub fn acquire(&self, id: &str) -> Result<SourceGuard> {
        let path = self.path(id, "lock")?;
        regular(&path)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        file.try_lock().map_err(|_| {
            DownloadError::new("source_busy", "Источник уже занят другой передачей")
        })?;
        Ok(SourceGuard { _file: file })
    }
    pub fn load(&self, id: &str) -> Result<Option<DownloadManifest>> {
        let mut latest: Option<DownloadManifest> = None;
        let mut invalid = false;
        for slot in 0..2 {
            let path = self.manifest_path(id, slot)?;
            regular(&path)?;
            if fs::metadata(&path).is_ok_and(|meta| meta.len() > 16 * 1024 * 1024) {
                return Err(DownloadError::new(
                    "manifest_limit",
                    "Слишком большой манифест",
                ));
            }
            let bytes = match fs::read(&path) {
                Ok(b) => b,
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.into()),
            };
            if bytes.len() > 16 * 1024 * 1024 {
                return Err(DownloadError::new(
                    "manifest_limit",
                    "Слишком большой манифест",
                ));
            }
            let manifest = match serde_json::from_slice::<DownloadManifest>(&bytes) {
                Ok(m) => m,
                Err(_) => {
                    invalid = true;
                    continue;
                }
            };
            if manifest.version != 1 {
                return Err(DownloadError::new(
                    "manifest_version_mismatch",
                    "Неподдерживаемая версия манифеста",
                ));
            }
            if manifest.request.source_id != id {
                return Err(DownloadError::new(
                    "manifest_source_mismatch",
                    "Манифест относится к другому источнику",
                ));
            }
            if latest
                .as_ref()
                .is_none_or(|old| manifest.generation > old.generation)
            {
                latest = Some(manifest);
            }
        }
        if invalid && latest.is_none() {
            return Err(DownloadError::new(
                "invalid_manifest",
                "Не удалось прочитать контрольную точку",
            ));
        }
        Ok(latest)
    }
    pub fn begin(&self, req: &DownloadRequest) -> Result<()> {
        self.path(&req.source_id, "part")?;
        if !matches!(req.url.scheme(), "http" | "https")
            || req.url.host_str().is_none()
            || !req.url.username().is_empty()
            || req.url.password().is_some()
        {
            return Err(DownloadError::new(
                "invalid_download_url",
                "Нужна прямая ссылка HTTP или HTTPS без встроенного пароля",
            ));
        }
        if req.expected_sha256.as_ref().is_some_and(|v| !valid_hash(v)) {
            return Err(DownloadError::new(
                "invalid_sha256",
                "SHA-256 должен содержать 64 шестнадцатеричных знака",
            ));
        }
        if let Some(old) = self.load(&req.source_id)? {
            if old.request.url != req.url || old.request.expected_sha256 != req.expected_sha256 {
                return Err(DownloadError::new(
                    "source_mismatch",
                    "Новая ссылка требует нового идентификатора источника",
                ));
            }
            return Ok(());
        }
        let path = self.part_path(&req.source_id)?;
        regular(&path)?;
        OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)?
            .sync_all()?;
        let manifest = DownloadManifest {
            version: 1,
            generation: 1,
            request: req.clone(),
            effective_url: None,
            validator: None,
            total_bytes: None,
            checkpoint: Checkpoint::default(),
            display_name: "Словарь".into(),
            completed: None,
            transfer_finished: false,
            statistics: crate::DownloadStatistics::default(),
        };
        self.save(&manifest)
    }
    pub fn request(&self, id: &str) -> Result<DownloadRequest> {
        Ok(self.required(id)?.request)
    }
    pub(crate) fn required(&self, id: &str) -> Result<DownloadManifest> {
        self.load(id)?
            .ok_or_else(|| DownloadError::new("source_not_found", "Источник не найден в кэше"))
    }
    pub(crate) fn save(&self, manifest: &DownloadManifest) -> Result<()> {
        let path =
            self.manifest_path(&manifest.request.source_id, (manifest.generation % 2) as u8)?;
        regular(&path)?;
        AtomicFile::new(path, AllowOverwrite)
            .write(|file| serde_json::to_writer(file, manifest).map_err(io::Error::other))
            .map_err(|_| {
                DownloadError::new(
                    "manifest_write_failed",
                    "Не удалось сохранить контрольную точку",
                )
            })
    }
    pub fn commit_checkpoint(&self, id: &str, checkpoint: Checkpoint) -> Result<()> {
        self.commit_checkpoint_with_hook(id, checkpoint, |_| Ok(()))
    }
    /// Точки отказа для тестирования durability, без подмены файловой системы.
    pub fn commit_checkpoint_with_hook(
        &self,
        id: &str,
        checkpoint: Checkpoint,
        hook: impl Fn(CommitPoint) -> io::Result<()>,
    ) -> Result<()> {
        let mut manifest = self.required(id)?;
        let path = self.part_path(id)?;
        regular(&path)?;
        let part = OpenOptions::new().read(true).write(true).open(path)?;
        let sum = checkpoint.chunk_hashes.iter().try_fold(0u64, |sum, h| {
            if h.bytes == 0 || !valid_hash(&h.sha256) {
                None
            } else {
                sum.checked_add(h.bytes)
            }
        });
        if sum != Some(checkpoint.confirmed_bytes)
            || part.metadata()?.len() < checkpoint.confirmed_bytes
        {
            return Err(DownloadError::new(
                "invalid_checkpoint",
                "Контрольная точка опережает сохранённые данные",
            ));
        }
        hook(CommitPoint::BeforeSync)?;
        part.sync_data()?;
        hook(CommitPoint::AfterSync)?;
        manifest.checkpoint = checkpoint;
        manifest.generation += 1;
        hook(CommitPoint::BeforeManifest)?;
        self.save(&manifest)?;
        hook(CommitPoint::AfterManifest)?;
        Ok(())
    }
    pub fn verify_prefix(&self, id: &str) -> Result<ResumePoint> {
        let manifest = self.required(id)?;
        let path = self.part_path(id)?;
        regular(&path)?;
        let mut part = OpenOptions::new().read(true).write(true).open(path)?;
        let mut total = 0u64;
        for chunk in &manifest.checkpoint.chunk_hashes {
            let mut hasher = Sha256::new();
            let read = io::copy(&mut (&mut part).take(chunk.bytes), &mut hasher)?;
            if read != chunk.bytes || format!("{:x}", hasher.finalize()) != chunk.sha256 {
                return Err(DownloadError::new(
                    "checkpoint_hash_mismatch",
                    "Сохранённая часть повреждена; требуется новая загрузка",
                ));
            }
            total = total.checked_add(read).ok_or_else(|| {
                DownloadError::new(
                    "invalid_checkpoint",
                    "Переполнение размера контрольной точки",
                )
            })?;
        }
        if total != manifest.checkpoint.confirmed_bytes {
            return Err(DownloadError::new(
                "invalid_checkpoint",
                "Некорректный размер контрольной точки",
            ));
        }
        part.set_len(total)?;
        part.sync_data()?;
        Ok(ResumePoint {
            confirmed_bytes: total,
            validator: manifest.validator,
            effective_url: manifest.effective_url,
            total_bytes: manifest.total_bytes,
        })
    }
    pub fn completed(&self, id: &str) -> Result<Option<CompletedDownload>> {
        let Some(manifest) = self.load(id)? else {
            return Ok(None);
        };
        let Some(done) = manifest.completed else {
            return Ok(None);
        };
        let path = self.part_path(id)?;
        regular(&path)?;
        let mut file = File::open(&path)?;
        let mut hash = Sha256::new();
        let bytes = io::copy(&mut file, &mut hash)?;
        if bytes != done.bytes || format!("{:x}", hash.finalize()) != done.sha256 {
            return Err(DownloadError::new(
                "completed_hash_mismatch",
                "Готовая локальная копия повреждена",
            ));
        }
        Ok(Some(CompletedDownload { path, ..done }))
    }
    pub fn remove(&self, id: &str) -> Result<()> {
        let _guard = self.acquire(id)?;
        for path in [
            self.part_path(id)?,
            self.manifest_path(id, 0)?,
            self.manifest_path(id, 1)?,
        ] {
            regular(&path)?;
            match fs::remove_file(path) {
                Ok(()) => (),
                Err(e) if e.kind() == io::ErrorKind::NotFound => (),
                Err(e) => return Err(e.into()),
            }
        }
        // Lock inode сохраняется, чтобы новый writer не обошёл активную блокировку.
        Ok(())
    }
}
fn valid_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|c| c.is_ascii_hexdigit())
}
