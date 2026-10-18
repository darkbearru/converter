use crate::{storage::PublishedResult, *};
use converter_core::model::{ConvertedArtifacts, PreparedFile};
use converter_download::DownloadRequest;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
};
#[derive(Clone, Serialize, Deserialize)]
pub enum RegisteredSource {
    Local(PreparedFile),
    Remote(DownloadRequest),
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdditionalDictionariesState {
    Pending,
    Declined,
    Accepted,
    Dispatched,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct AdditionalDictionaryChild {
    pub job_id: String,
    pub dictionary: InputChoice,
    #[serde(default)]
    pub fingerprint: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct StoredAdditionalDictionaries {
    pub offer: AdditionalDictionariesOffer,
    pub state: AdditionalDictionariesState,
    pub children: Vec<AdditionalDictionaryChild>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct StoredJob {
    pub version: u32,
    pub draft: JobDraft,
    pub sources: Vec<RegisteredSource>,
    pub snapshot: JobSnapshot,
    pub published: Option<PublishedResult>,
    pub selected_main: Option<String>,
    #[serde(default)]
    pub selected_main_fingerprint: Option<String>,
    pub ready: Option<ConvertedArtifacts>,
    #[serde(default)]
    pub staging_path: Option<PathBuf>,
    #[serde(default)]
    pub additional_dictionaries: Option<StoredAdditionalDictionaries>,
}
pub struct Store {
    pub root: PathBuf,
    _lock: File,
}
impl Store {
    pub fn cleanup_staging(&self, job: &mut StoredJob) -> AppResult<()> {
        if job.ready.is_some() {
            return Ok(());
        }
        let Some(path) = &job.staging_path else {
            return Ok(());
        };
        let allowed = match &job.draft.output {
            OutputChoice::DesktopFolder(path) => path.clone(),
            OutputChoice::AppLibrary => self.root.join("results"),
        };
        let Ok(allowed) = allowed.canonicalize() else {
            return Ok(());
        };
        let prefix = format!(".converter-{}-", job.snapshot.id);
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with(&prefix))
            && path.parent().and_then(|p| p.canonicalize().ok()) == Some(allowed)
        {
            match fs::symlink_metadata(path) {
                Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {
                    fs::remove_dir_all(path)?
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => (),
                _ => {
                    return Err(AppError::new(
                        "invalid_staging",
                        "Некорректная временная директория задания",
                    ));
                }
            }
            job.staging_path = None;
        }
        Ok(())
    }
    pub fn open(root: PathBuf) -> AppResult<Self> {
        fs::create_dir_all(&root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        }
        for folder in ["jobs", "downloads", "scratch", "results"] {
            fs::create_dir_all(root.join(folder))?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("app.lock"))?;
        file.try_lock()
            .map_err(|_| AppError::new("app_busy", "Это хранилище уже открыто другим процессом"))?;
        Ok(Self { root, _lock: file })
    }
    pub fn id_path(&self, id: &str, suffix: &str) -> AppResult<PathBuf> {
        uuid::Uuid::parse_str(id)
            .map_err(|_| AppError::new("invalid_job_id", "Задание не найдено"))?;
        if id.len() != 36 {
            return Err(AppError::new("invalid_job_id", "Задание не найдено"));
        }
        Ok(self.root.join("jobs").join(format!("{id}.{suffix}")))
    }
    pub fn save(&self, job: &StoredJob) -> AppResult<()> {
        write_json(&self.id_path(&job.snapshot.id, "json")?, job)
    }
    pub fn load(&self) -> AppResult<BTreeMap<String, StoredJob>> {
        let mut jobs = BTreeMap::new();
        for entry in fs::read_dir(self.root.join("jobs"))? {
            let entry = entry?;
            if entry.path().extension().and_then(|v| v.to_str()) != Some("json") {
                continue;
            }
            if !entry.file_type()?.is_file() {
                return Err(AppError::new("invalid_store", "Некорректный файл очереди"));
            }
            let job: StoredJob = serde_json::from_reader(File::open(entry.path())?)?;
            if job.version != 1 || entry.path() != self.id_path(&job.snapshot.id, "json")? {
                return Err(AppError::new(
                    "store_version_mismatch",
                    "Неподдерживаемая версия очереди",
                ));
            }
            jobs.insert(job.snapshot.id.clone(), job);
        }
        Ok(jobs)
    }
    pub fn imports(&self) -> AppResult<BTreeMap<String, PreparedFile>> {
        let path = self.root.join("imports.json");
        match File::open(path) {
            Ok(file) => Ok(serde_json::from_reader(file)?),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(e.into()),
        }
    }
    pub fn save_imports(&self, imports: &BTreeMap<String, PreparedFile>) -> AppResult<()> {
        write_json(&self.root.join("imports.json"), imports)
    }
}
fn write_json(path: &Path, value: &impl Serialize) -> AppResult<()> {
    atomicwrites::AtomicFile::new(path, atomicwrites::OverwriteBehavior::AllowOverwrite)
        .write(|file| serde_json::to_writer(file, value).map_err(io::Error::other))
        .map_err(|_| AppError::new("store_write_failed", "Не удалось сохранить очередь"))
}
