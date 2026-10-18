use super::{
    progress::{ProgressGate, aggregate_downloads},
    scheduler::Controls,
    state,
    store::{RegisteredSource, Store, StoredJob},
};
use crate::{storage::*, *};
use converter_core::{model::*, pipeline::convert_to_staging, prepare::prepare_source};
use converter_download::{
    DownloadCache, DownloadManager, DownloadPolicy, DownloadProgress, DownloadRequest,
    TransferOutcome, TransferState,
};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Read, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
use tokio::sync::{Notify, Semaphore, broadcast};

struct Runtime {
    jobs: BTreeMap<String, StoredJob>,
    imports: BTreeMap<String, PreparedFile>,
    controls: BTreeMap<String, Controls>,
    gates: BTreeMap<String, ProgressGate>,
}
struct Inner {
    store: Store,
    storage: Arc<dyn PlatformStorage>,
    runtime: Mutex<Runtime>,
    events: broadcast::Sender<JobEvent>,
    transfer: Semaphore,
    conversion: Semaphore,
    epoch: Instant,
    stopping: AtomicBool,
    idle: Notify,
    journals: Mutex<BTreeMap<String, Arc<Mutex<BufWriter<File>>>>>,
    cache: DownloadCache,
}
#[derive(Clone)]
pub struct JobManager {
    inner: Arc<Inner>,
}
fn lock_error() -> AppError {
    AppError::new("state_lock", "Не удалось получить состояние очереди")
}
fn cancel_download_progress(snapshot: &mut JobSnapshot) {
    for file in &mut snapshot.download.files {
        if file.state != TransferState::Completed {
            file.state = TransferState::Cancelled;
        }
        file.bytes_per_second = None;
    }
    snapshot.download = aggregate_downloads(snapshot.download.files.clone());
}
impl JobManager {
    pub fn open(data_dir: PathBuf, storage: Arc<dyn PlatformStorage>) -> AppResult<Self> {
        let store = Store::open(data_dir)?;
        let mut jobs = store.load()?;
        let imports = store.imports()?;
        let cache = DownloadCache::open(store.root.join("downloads"))?;
        for job in jobs.values_mut() {
            let snapshot = &mut job.snapshot;
            if matches!(
                snapshot.state,
                JobState::Downloading | JobState::DownloadRetryWait
            ) {
                snapshot.state = JobState::DownloadPaused;
                for file in &mut snapshot.download.files {
                    if let Some(manifest) = cache.load(&file.source_id)? {
                        file.confirmed_bytes = manifest.confirmed_bytes();
                        file.total_bytes = manifest.total_bytes();
                    }
                    if file.state != TransferState::Completed {
                        file.state = TransferState::Paused;
                        file.received_bytes = file.confirmed_bytes;
                        file.bytes_per_second = None;
                    }
                }
                snapshot.download = aggregate_downloads(snapshot.download.files.clone());
                snapshot.sequence += 1;
            } else if snapshot.state == JobState::Cancelled
                && snapshot.download.files.iter().any(|file| {
                    (file.state != TransferState::Completed
                        && file.state != TransferState::Cancelled)
                        || file.bytes_per_second.is_some()
                })
            {
                cancel_download_progress(snapshot);
                snapshot.sequence += 1;
            } else if state::active(snapshot.state) {
                snapshot.state = JobState::Interrupted;
                snapshot.conversion.status = ConversionStatus::Interrupted;
                snapshot.sequence += 1;
            }
            // Только выделенная scratch-директория приложения; кэш загрузок не затрагивается.
            let scratch = store.root.join("scratch").join(&snapshot.id);
            if scratch.exists() {
                fs::remove_dir_all(scratch)?;
            }
            store.cleanup_staging(job)?;
            store.save(job)?;
        }
        let (events, _) = broadcast::channel(256);
        Ok(Self {
            inner: Arc::new(Inner {
                store,
                storage,
                runtime: Mutex::new(Runtime {
                    jobs,
                    imports,
                    controls: BTreeMap::new(),
                    gates: BTreeMap::new(),
                }),
                events,
                transfer: Semaphore::new(1),
                conversion: Semaphore::new(1),
                epoch: Instant::now(),
                stopping: AtomicBool::new(false),
                idle: Notify::new(),
                journals: Mutex::new(BTreeMap::new()),
                cache,
            }),
        })
    }
    pub fn subscribe(&self) -> broadcast::Receiver<JobEvent> {
        self.inner.events.subscribe()
    }
    pub fn list(&self) -> AppResult<Vec<JobSnapshot>> {
        Ok(self
            .inner
            .runtime
            .lock()
            .map_err(|_| lock_error())?
            .jobs
            .values()
            .map(|j| j.snapshot.clone())
            .collect())
    }
    fn job(&self, id: &str) -> AppResult<StoredJob> {
        self.inner
            .runtime
            .lock()
            .map_err(|_| lock_error())?
            .jobs
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::new("job_not_found", "Задание не найдено"))
    }
    pub async fn import(&self, selection: PlatformSelection) -> AppResult<Vec<ImportedSourceInfo>> {
        let ctx = RunContext::new(self.inner.store.root.join("scratch/import"));
        let files = self.inner.storage.import(selection, &ctx).await?;
        let mut runtime = self.inner.runtime.lock().map_err(|_| lock_error())?;
        let mut result = Vec::new();
        for mut file in files {
            file.id = uuid::Uuid::new_v4().to_string();
            let meta = fs::metadata(&file.path)?;
            result.push(ImportedSourceInfo {
                import_id: file.id.clone(),
                display_name: file.display_name.clone(),
                bytes: meta.is_file().then_some(meta.len()),
                is_directory: meta.is_dir(),
            });
            runtime.imports.insert(file.id.clone(), file);
        }
        self.inner.store.save_imports(&runtime.imports)?;
        Ok(result)
    }
    fn register(&self, input: &SourceInput) -> AppResult<RegisteredSource> {
        match input {
            SourceInput::Local { import_id } => self
                .inner
                .runtime
                .lock()
                .map_err(|_| lock_error())?
                .imports
                .get(import_id)
                .cloned()
                .map(RegisteredSource::Local)
                .ok_or_else(|| AppError::new("source_not_found", "Выберите исходный файл заново")),
            SourceInput::Remote {
                url,
                expected_sha256,
            } => {
                let url = url.parse::<url::Url>().map_err(|_| {
                    AppError::new("invalid_download_url", "Некорректная ссылка HTTP/HTTPS")
                })?;
                if !matches!(url.scheme(), "http" | "https")
                    || url.host_str().is_none()
                    || !url.username().is_empty()
                    || url.password().is_some()
                {
                    return Err(AppError::new(
                        "invalid_download_url",
                        "Нужна прямая ссылка HTTP/HTTPS без встроенного пароля",
                    ));
                }
                if expected_sha256
                    .as_ref()
                    .is_some_and(|h| h.len() != 64 || !h.bytes().all(|b| b.is_ascii_hexdigit()))
                {
                    return Err(AppError::new(
                        "invalid_sha256",
                        "SHA-256 должен содержать 64 шестнадцатеричных знака",
                    ));
                }
                Ok(RegisteredSource::Remote(DownloadRequest {
                    source_id: uuid::Uuid::new_v4().to_string(),
                    url,
                    expected_sha256: expected_sha256.clone(),
                }))
            }
        }
    }
    pub async fn create(&self, draft: JobDraft) -> AppResult<JobSnapshot> {
        let mut sources = vec![self.register(&draft.source)?];
        sources.extend(
            draft
                .resources
                .iter()
                .map(|source| self.register(source))
                .collect::<AppResult<Vec<_>>>()?,
        );
        let display_name = match &sources[0] {
            RegisteredSource::Local(file) => file.display_name.clone(),
            RegisteredSource::Remote(request) => request
                .url
                .host_str()
                .unwrap_or("Словарь по ссылке")
                .to_owned(),
        };
        let files = sources
            .iter()
            .filter_map(|source| match source {
                RegisteredSource::Remote(request) => Some(DownloadProgress {
                    source_id: request.source_id.clone(),
                    state: TransferState::Pending,
                    received_bytes: 0,
                    confirmed_bytes: 0,
                    total_bytes: None,
                    bytes_per_second: None,
                    restart_reason: None,
                }),
                _ => None,
            })
            .collect();
        let id = uuid::Uuid::new_v4().to_string();
        let snapshot = JobSnapshot {
            id: id.clone(),
            display_name,
            generation: 0,
            run_id: uuid::Uuid::new_v4().to_string(),
            sequence: 0,
            state: JobState::Queued,
            download: aggregate_downloads(files),
            conversion: ConversionJobProgress::default(),
            result: None,
            export: ExportSnapshot::default(),
            error: None,
            input_choices: vec![],
            diagnostic_count: 0,
            options: draft.options.clone(),
            metadata: None,
        };
        let auto = draft.auto_start;
        let job = StoredJob {
            version: 1,
            draft,
            sources,
            snapshot: snapshot.clone(),
            published: None,
            selected_main: None,
            ready: None,
            staging_path: None,
        };
        self.inner.store.save(&job)?;
        self.inner
            .runtime
            .lock()
            .map_err(|_| lock_error())?
            .jobs
            .insert(id.clone(), job);
        let _ = self.inner.events.send(JobEvent {
            snapshot: snapshot.clone(),
        });
        if auto {
            self.start(&id).await
        } else {
            Ok(snapshot)
        }
    }
    fn mutate(
        &self,
        id: &str,
        expected: Option<(u64, &str)>,
        force: bool,
        update: impl FnOnce(&mut StoredJob),
    ) -> AppResult<Option<JobSnapshot>> {
        let mut runtime = self.inner.runtime.lock().map_err(|_| lock_error())?;
        let mut job = runtime
            .jobs
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::new("job_not_found", "Задание не найдено"))?;
        if expected.is_some_and(|(generation, run)| {
            generation != job.snapshot.generation || run != job.snapshot.run_id
        }) {
            return Ok(None);
        }
        update(&mut job);
        job.snapshot.sequence += 1;
        let key = format!(
            "{:?}/{:?}/{:?}",
            job.snapshot.state,
            job.snapshot.conversion.current.as_ref().map(|p| p.stage),
            job.snapshot
                .download
                .files
                .iter()
                .map(|f| (f.source_id.as_str(), f.state))
                .collect::<Vec<_>>()
        );
        let emit = runtime.gates.entry(id.into()).or_default().should_emit(
            self.inner.epoch.elapsed(),
            &key,
            force,
        );
        if emit {
            self.inner.store.save(&job)?;
            let _ = self.inner.events.send(JobEvent {
                snapshot: job.snapshot.clone(),
            });
        }
        let snapshot = job.snapshot.clone();
        runtime.jobs.insert(id.into(), job);
        Ok(Some(snapshot))
    }
    pub async fn start(&self, id: &str) -> AppResult<JobSnapshot> {
        if self.inner.stopping.load(Ordering::SeqCst) {
            return Err(AppError::new("app_stopping", "Приложение завершает работу"));
        }
        if matches!(
            self.job(id)?.snapshot.state,
            JobState::Succeeded
                | JobState::SucceededWithWarnings
                | JobState::DownloadPaused
                | JobState::DownloadFailed
                | JobState::Failed
                | JobState::Cancelled
                | JobState::Interrupted
        ) {
            self.wait_idle(id).await?;
        }
        let controls = Controls::default();
        let snapshot = {
            let mut runtime = self.inner.runtime.lock().map_err(|_| lock_error())?;
            if runtime.controls.contains_key(id) {
                return Err(AppError::new("job_busy", "Задание уже выполняется"));
            }
            let mut job = runtime
                .jobs
                .get(id)
                .cloned()
                .ok_or_else(|| AppError::new("job_not_found", "Задание не найдено"))?;
            if job.snapshot.state == JobState::AwaitingInput {
                return Err(AppError::new(
                    "input_required",
                    "Сначала уточните параметры задания",
                ));
            }
            job.snapshot.generation += 1;
            job.snapshot.run_id = uuid::Uuid::new_v4().to_string();
            job.snapshot.sequence = 0;
            job.snapshot.state = JobState::Queued;
            job.snapshot.error = None;
            job.snapshot.result = None;
            if job.ready.is_none() {
                job.snapshot.conversion = ConversionJobProgress::default();
                job.snapshot.diagnostic_count = 0;
            }
            let path = self.inner.store.id_path(id, "diagnostics.jsonl")?;
            let journal = if job.ready.is_some() {
                fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)?
            } else {
                File::create(path)?
            };
            self.inner
                .journals
                .lock()
                .map_err(|_| lock_error())?
                .insert(id.into(), Arc::new(Mutex::new(BufWriter::new(journal))));
            self.inner.store.save(&job)?;
            runtime.controls.insert(id.into(), controls.clone());
            let snapshot = job.snapshot.clone();
            runtime.jobs.insert(id.into(), job);
            snapshot
        };
        let _ = self.inner.events.send(JobEvent {
            snapshot: snapshot.clone(),
        });
        let manager = self.clone();
        let id = id.to_owned();
        let run = snapshot.run_id.clone();
        let generation = snapshot.generation;
        tokio::spawn(async move {
            let result = manager.process(&id, generation, &run, controls).await;
            if let Err(error) = result {
                let shutdown = manager.inner.stopping.load(Ordering::SeqCst);
                let _ = manager.mutate(&id, Some((generation, &run)), true, |job| {
                    if error.code == "cancelled" {
                        job.snapshot.state = if shutdown {
                            JobState::Interrupted
                        } else {
                            JobState::Cancelled
                        };
                        job.snapshot.conversion.status = if shutdown {
                            ConversionStatus::Interrupted
                        } else {
                            ConversionStatus::Cancelled
                        };
                    } else if job.ready.is_some()
                        || matches!(
                            error.code.as_str(),
                            "encoding_required"
                                | "ambiguous_resource"
                                | "ambiguous_abbreviations"
                                | "output_unavailable"
                        )
                    {
                        job.snapshot.state = JobState::AwaitingInput;
                    } else if job.snapshot.state != JobState::DownloadFailed {
                        job.snapshot.state = JobState::Failed;
                        job.snapshot.conversion.status = ConversionStatus::Failed;
                    }
                    job.snapshot.error = Some(error);
                });
            }
            if let Ok(journals) = manager.inner.journals.lock()
                && let Some(journal) = journals.get(&id)
                && let Ok(mut out) = journal.lock()
            {
                let _ = out.flush();
            }
            if let Ok(mut runtime) = manager.inner.runtime.lock() {
                runtime.controls.remove(&id);
            }
            let _ = fs::remove_dir_all(manager.inner.store.root.join("scratch").join(&id));
            manager.inner.idle.notify_waiters();
        });
        Ok(snapshot)
    }
    pub async fn pause_download(&self, id: &str) -> AppResult<JobSnapshot> {
        let controls = self
            .inner
            .runtime
            .lock()
            .map_err(|_| lock_error())?
            .controls
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::new("download_not_active", "Скачивание уже остановлено"))?;
        let job = self.job(id)?;
        if !matches!(
            job.snapshot.state,
            JobState::Queued | JobState::Downloading | JobState::DownloadRetryWait
        ) || job.snapshot.download.kind != DownloadKind::Remote
        {
            return Err(AppError::new(
                "download_not_active",
                "Скачивание сейчас не выполняется",
            ));
        }
        controls.download.pause();
        Ok(self
            .mutate(id, None, true, |job| {
                job.snapshot.state = JobState::DownloadPaused;
                for file in &mut job.snapshot.download.files {
                    file.bytes_per_second = None;
                }
            })?
            .expect("current job"))
    }
    pub async fn resume_download(&self, id: &str) -> AppResult<JobSnapshot> {
        self.wait_idle(id).await?;
        self.start(id).await
    }
    async fn wait_idle(&self, id: &str) -> AppResult<()> {
        // Дождаться выхода предыдущего worker после записи checkpoint.
        loop {
            let notified = self.inner.idle.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if !self
                .inner
                .runtime
                .lock()
                .map_err(|_| lock_error())?
                .controls
                .contains_key(id)
            {
                break;
            }
            notified.await;
        }
        Ok(())
    }
    pub async fn cancel(&self, id: &str) -> AppResult<JobSnapshot> {
        let current = self.job(id)?.snapshot;
        if state::success(current.state) {
            return Ok(current);
        }
        if let Some(controls) = self
            .inner
            .runtime
            .lock()
            .map_err(|_| lock_error())?
            .controls
            .get(id)
        {
            controls.download.cancel();
            controls.conversion.cancel();
        }
        Ok(self
            .mutate(id, None, true, |job| {
                if state::success(job.snapshot.state) {
                    return;
                }
                job.snapshot.state = JobState::Cancelled;
                job.snapshot.conversion.status = ConversionStatus::Cancelled;
                cancel_download_progress(&mut job.snapshot);
            })?
            .expect("current job"))
    }
    pub async fn update(&self, id: &str, update: JobOptionsUpdate) -> AppResult<JobSnapshot> {
        if self.job(id)?.snapshot.state != JobState::AwaitingInput {
            return Err(AppError::new(
                "job_not_awaiting",
                "Параметры можно менять, пока задание ожидает ввода",
            ));
        }
        self.wait_idle(id).await?;
        let mut update = update;
        let resources = update
            .resources
            .as_ref()
            .map(|ids| {
                ids.iter()
                    .map(|id| {
                        self.register(&SourceInput::Local {
                            import_id: id.clone(),
                        })
                    })
                    .collect::<AppResult<Vec<_>>>()
            })
            .transpose()?;
        let mut child_ids = Vec::new();
        if update.dictionary_id.as_deref() == Some("*") {
            let template = self.job(id)?;
            let first = template.snapshot.input_choices.first().ok_or_else(|| {
                AppError::new(
                    "dictionary_selection_required",
                    "В пакете нет списка словарей",
                )
            })?;
            update.dictionary_id = Some(first.id.clone());
            for choice in template.snapshot.input_choices.iter().skip(1) {
                let mut child = template.clone();
                let id = uuid::Uuid::new_v4().to_string();
                child.snapshot.id = id.clone();
                child.snapshot.display_name = choice.display_name.clone();
                child.snapshot.generation = 0;
                child.snapshot.run_id = uuid::Uuid::new_v4().to_string();
                child.snapshot.sequence = 0;
                child.snapshot.state = JobState::Queued;
                child.snapshot.input_choices.clear();
                child.snapshot.error = None;
                child.selected_main = Some(choice.id.clone());
                child.ready = None;
                child.staging_path = None;
                if let Some(options) = &update.options {
                    child.draft.options = options.clone();
                    child.snapshot.options = options.clone();
                }
                if let Some(metadata) = &update.metadata_override {
                    child.draft.metadata_override = Some(metadata.clone());
                }
                if let Some(output) = &update.output {
                    child.draft.output = output.clone();
                }
                if let Some(resources) = &resources {
                    child.sources.truncate(1);
                    child.sources.extend(resources.clone());
                }
                self.inner.store.save(&child)?;
                self.inner
                    .runtime
                    .lock()
                    .map_err(|_| lock_error())?
                    .jobs
                    .insert(id.clone(), child);
                child_ids.push(id);
            }
        }
        let snapshot = self
            .mutate(id, None, true, |job| {
                if let Some(options) = update.options {
                    if serde_json::to_value(&options).ok()
                        != serde_json::to_value(&job.draft.options).ok()
                    {
                        job.ready = None;
                    }
                    job.draft.options = options;
                    job.snapshot.options = job.draft.options.clone();
                }
                if let Some(metadata) = update.metadata_override {
                    if serde_json::to_value(&metadata).ok()
                        != serde_json::to_value(&job.draft.metadata_override).ok()
                    {
                        job.ready = None;
                    }
                    job.draft.metadata_override = Some(metadata);
                }
                if let Some(output) = update.output {
                    job.draft.output = output;
                }
                if let Some(id) = update.dictionary_id {
                    job.selected_main = Some(id);
                }
                if let Some(resources) = resources {
                    job.ready = None;
                    job.sources.truncate(1);
                    job.sources.extend(resources);
                    let ids: Vec<_> = job
                        .sources
                        .iter()
                        .filter_map(|s| match s {
                            RegisteredSource::Remote(r) => Some(r.source_id.as_str()),
                            _ => None,
                        })
                        .collect();
                    job.snapshot.download = aggregate_downloads(
                        job.snapshot
                            .download
                            .files
                            .iter()
                            .filter(|f| ids.contains(&f.source_id.as_str()))
                            .cloned()
                            .collect(),
                    );
                }
                job.snapshot.error = None;
                job.snapshot.state = JobState::Queued;
            })?
            .expect("current job");
        for child in child_ids {
            self.start(&child).await?;
        }
        Ok(snapshot)
    }
    pub async fn shutdown(&self) -> AppResult<()> {
        self.inner.stopping.store(true, Ordering::SeqCst);
        {
            let runtime = self.inner.runtime.lock().map_err(|_| lock_error())?;
            for controls in runtime.controls.values() {
                controls.download.pause();
                controls.conversion.cancel();
            }
        }
        loop {
            let notified = self.inner.idle.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self
                .inner
                .runtime
                .lock()
                .map_err(|_| lock_error())?
                .controls
                .is_empty()
            {
                break;
            }
            notified.await;
        }
        Ok(())
    }
    pub fn accept_core_event(
        &self,
        id: &str,
        generation: u64,
        run: &str,
        event: CoreEvent,
    ) -> AppResult<bool> {
        let job = self.job(id)?;
        if job.snapshot.generation != generation
            || job.snapshot.run_id != run
            || state::success(job.snapshot.state)
            || matches!(
                job.snapshot.state,
                JobState::Cancelled | JobState::Interrupted
            )
        {
            return Ok(false);
        }
        if let CoreEvent::Diagnostic(diagnostic) = &event
            && let Some(journal) = self
                .inner
                .journals
                .lock()
                .map_err(|_| lock_error())?
                .get(id)
        {
            let mut out = journal.lock().map_err(|_| lock_error())?;
            serde_json::to_writer(&mut *out, diagnostic)?;
            out.write_all(b"\n")?;
        }
        let terminal = matches!(event, CoreEvent::StageCompleted(_));
        self.mutate(id, Some((generation, run)), terminal, |job| match event {
            CoreEvent::Diagnostic(_) => job.snapshot.diagnostic_count += 1,
            CoreEvent::Progress(progress) | CoreEvent::StageCompleted(progress) => {
                job.snapshot.state = match progress.stage {
                    ConversionStage::Inspect => JobState::Inspecting,
                    ConversionStage::Validate => JobState::Validating,
                    ConversionStage::Publish => JobState::Publishing,
                    _ => JobState::Converting,
                };
                job.snapshot.conversion.status = ConversionStatus::Running;
                job.snapshot.conversion.current = Some(progress.clone());
                if terminal {
                    if let Some(old) = job
                        .snapshot
                        .conversion
                        .completed_stages
                        .iter_mut()
                        .find(|p| p.stage == progress.stage)
                    {
                        *old = progress;
                    } else {
                        job.snapshot.conversion.completed_stages.push(progress);
                    }
                }
            }
        })?;
        Ok(true)
    }
    pub fn diagnostics(&self, id: &str, offset: u64, limit: u32) -> AppResult<DiagnosticPage> {
        let job = self.job(id)?;
        if let Some(journal) = self
            .inner
            .journals
            .lock()
            .map_err(|_| lock_error())?
            .get(id)
        {
            journal.lock().map_err(|_| lock_error())?.flush()?;
        }
        let file = match File::open(self.inner.store.id_path(id, "diagnostics.jsonl")?) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(DiagnosticPage {
                    items: vec![],
                    next_offset: None,
                    total: job.snapshot.diagnostic_count,
                });
            }
            Err(e) => return Err(e.into()),
        };
        let mut items = Vec::new();
        let limit = limit.clamp(1, 200) as usize;
        for (index, line) in BufReader::new(file).lines().enumerate() {
            if (index as u64) < offset {
                continue;
            }
            items.push(serde_json::from_str(&line?)?);
            if items.len() > limit {
                break;
            }
        }
        let next_offset = if items.len() > limit {
            items.pop();
            Some(offset + limit as u64)
        } else {
            None
        };
        Ok(DiagnosticPage {
            items,
            next_offset,
            total: job.snapshot.diagnostic_count,
        })
    }
    pub async fn export(&self, id: &str, choice: ExportChoice) -> AppResult<ExportSnapshot> {
        let job = self.job(id)?;
        let result = job
            .published
            .ok_or_else(|| AppError::new("result_not_found", "Готовый результат ещё не создан"))?;
        let context = RunContext::new(
            self.inner
                .store
                .root
                .join("scratch")
                .join(id)
                .join("export"),
        );
        let exported = match self.inner.storage.export(&result, choice, &context).await {
            Ok(exported) => exported,
            Err(error) => ExportSnapshot {
                status: ExportStatus::Failed,
                saved_files: vec![],
                error: Some(error),
            },
        };
        self.mutate(id, None, true, |job| job.snapshot.export = exported.clone())?;
        Ok(exported)
    }
    pub fn result_directory(&self, id: &str) -> AppResult<PathBuf> {
        self.job(id)?
            .published
            .map(|p| p.directory)
            .ok_or_else(|| AppError::new("result_not_found", "Готовый результат ещё не создан"))
    }
    pub fn remove_cached_source(&self, source_id: &str) -> AppResult<()> {
        let runtime = self.inner.runtime.lock().map_err(|_| lock_error())?;
        if runtime.jobs.values().any(|job| job.sources.iter().any(|s| matches!(s, RegisteredSource::Remote(request) if request.source_id == source_id))) { return Err(AppError::new("source_in_use", "Источник используется заданием очереди")); }
        self.inner.cache.remove(source_id)?;
        Ok(())
    }
    async fn process(
        &self,
        id: &str,
        generation: u64,
        run: &str,
        controls: Controls,
    ) -> AppResult<()> {
        let job = self.job(id)?;
        if let Some(artifacts) = job.ready.clone() {
            let _permit = self
                .inner
                .conversion
                .acquire()
                .await
                .map_err(|_| lock_error())?;
            let (context, failure) = self.run_context(id, generation, run, &controls)?;
            let result = self
                .publish_result(id, generation, run, artifacts, &job.draft.output, &context)
                .await;
            if let Some(error) = failure.lock().map_err(|_| lock_error())?.take() {
                return Err(error);
            }
            return result;
        }
        let mut files = Vec::new();
        let mut audits = Vec::new();
        for source in &job.sources {
            match source {
                RegisteredSource::Local(file) => {
                    audits.push(super::report::SourceAudit::local(file)?);
                    files.push(file.clone());
                }
                RegisteredSource::Remote(request) => {
                    let permit = tokio::select! { biased; _ = controls.download.wait_for_stop() => { self.stopped_download(id, generation, run, &controls)?; return Ok(()); }, permit = self.inner.transfer.acquire() => permit.map_err(|_| lock_error())? };
                    let manager =
                        DownloadManager::new(self.inner.cache.clone(), DownloadPolicy::default())?;
                    let app = self.clone();
                    let (job_id, run_id) = (id.to_owned(), run.to_owned());
                    let callback_error = Arc::new(Mutex::new(None));
                    let failure = callback_error.clone();
                    let stop = controls.download.clone();
                    let outcome = manager
                        .fetch(
                            request.clone(),
                            controls.download.clone(),
                            Arc::new(move |progress| {
                                let result = app.mutate(
                                    &job_id,
                                    Some((generation, &run_id)),
                                    false,
                                    |job| {
                                        if (job.snapshot.state == JobState::Cancelled
                                            && progress.state != TransferState::Cancelled)
                                            || (job.snapshot.state == JobState::DownloadPaused
                                                && !matches!(
                                                    progress.state,
                                                    TransferState::Paused
                                                        | TransferState::Cancelled
                                                ))
                                        {
                                            return;
                                        }
                                        job.snapshot.state = match progress.state {
                                            TransferState::RetryWait => JobState::DownloadRetryWait,
                                            TransferState::Paused => JobState::DownloadPaused,
                                            TransferState::Failed => JobState::DownloadFailed,
                                            TransferState::Cancelled => JobState::Cancelled,
                                            _ => JobState::Downloading,
                                        };
                                        if let Some(file) = job
                                            .snapshot
                                            .download
                                            .files
                                            .iter_mut()
                                            .find(|f| f.source_id == progress.source_id)
                                        {
                                            *file = progress;
                                        }
                                        job.snapshot.download = aggregate_downloads(
                                            job.snapshot.download.files.clone(),
                                        );
                                    },
                                );
                                if let Err(error) = result {
                                    if let Ok(mut failure) = failure.lock() {
                                        *failure = Some(error);
                                    }
                                    stop.pause();
                                }
                            }),
                        )
                        .await;
                    drop(permit);
                    if let Some(error) = callback_error.lock().map_err(|_| lock_error())?.take() {
                        return Err(error);
                    }
                    match outcome? {
                        TransferOutcome::Completed(done) => {
                            audits.push(super::report::SourceAudit::remote(done.clone()));
                            files.push(PreparedFile {
                                id: done.source_id,
                                display_name: done.display_name,
                                path: done.path,
                            });
                        }
                        TransferOutcome::Paused | TransferOutcome::Cancelled => return Ok(()),
                    }
                }
            }
        }
        controls.conversion.check()?;
        let _permit = tokio::select! { biased; _ = controls.download.wait_for_stop() => { self.stopped_download(id, generation, run, &controls)?; return Ok(()); }, permit = self.inner.conversion.acquire() => permit.map_err(|_| lock_error())? };
        self.mutate(id, Some((generation, run)), true, |job| {
            job.snapshot.state = JobState::Importing
        })?;
        let (context, callback_error) = self.run_context(id, generation, run, &controls)?;
        let mut prepared = files.into_iter();
        let main = prepared
            .next()
            .ok_or_else(|| AppError::new("source_not_found", "Не выбран основной источник"))?;
        let attachments: Vec<_> = prepared.collect();
        // HTML определяется до запуска DSL даже для URL без расширения.
        let mut prefix = [0u8; 1024];
        let n = File::open(&main.path)?.read(&mut prefix)?;
        let text = String::from_utf8_lossy(&prefix[..n])
            .trim_start()
            .to_ascii_lowercase();
        if text.starts_with("<!doctype html") || text.starts_with("<html") {
            return Err(AppError::new(
                "download_is_html",
                "Вместо словаря получена HTML-страница. Нужна прямая ссылка на файл",
            ));
        }
        let bundle_context = context.clone();
        let bundles = tokio::task::spawn_blocking(move || {
            prepare_source(&main, &attachments, &bundle_context)
        })
        .await
        .map_err(|_| AppError::new("worker_failed", "Не удалось подготовить источник"))??;
        let selected = job.selected_main.as_deref();
        if bundles.len() > 1 && selected.is_none() {
            self.mutate(id, Some((generation, run)), true, |job| {
                job.snapshot.input_choices = bundles
                    .iter()
                    .map(|b| InputChoice {
                        id: b.main.id.clone(),
                        display_name: b.main.display_name.clone(),
                    })
                    .collect();
                job.snapshot.state = JobState::AwaitingInput;
            })?;
            return Ok(());
        }
        let bundle = if let Some(selected) = selected {
            bundles
                .into_iter()
                .find(|b| b.main.id == selected)
                .ok_or_else(|| {
                    AppError::new(
                        "dictionary_selection_required",
                        "Выберите словарь из пакета",
                    )
                })?
        } else {
            bundles
                .into_iter()
                .next()
                .ok_or_else(|| AppError::new("source_not_found", "В пакете нет словаря"))?
        };
        let inspected_bundle = bundle.clone();
        let inspected_options = job.draft.options.clone();
        let inspected_context = context.clone();
        let metadata = tokio::task::spawn_blocking(
            move || -> converter_core::model::Result<DictionaryMetadata> {
                let registry = converter_core::formats::FormatRegistry::builtins();
                let mut prefix = Vec::new();
                File::open(&inspected_bundle.main.path)?
                    .take(64 * 1024)
                    .read_to_end(&mut prefix)?;
                let format = registry.detect(&prefix, &inspected_bundle.main.display_name)?;
                registry.get(&format)?.inspect(
                    &inspected_bundle,
                    &inspected_options,
                    &inspected_context,
                )
            },
        )
        .await
        .map_err(|_| AppError::new("worker_failed", "Не удалось прочитать параметры словаря"))??;
        self.mutate(id, Some((generation, run)), true, |job| {
            job.snapshot.state = JobState::Converting;
            job.snapshot.conversion.status = ConversionStatus::Running;
            job.snapshot.display_name = bundle.main.display_name.clone();
            job.snapshot.metadata = Some(metadata.clone());
            if let (Some(info), Some(overrides)) =
                (&mut job.snapshot.metadata, &job.draft.metadata_override)
            {
                if !overrides.title.is_empty() {
                    info.title.clone_from(&overrides.title);
                }
                info.description.clone_from(&overrides.description);
                if !overrides.index_language.is_empty() {
                    info.index_language.clone_from(&overrides.index_language);
                }
                if !overrides.contents_language.is_empty() {
                    info.contents_language
                        .clone_from(&overrides.contents_language);
                }
            }
            job.snapshot.input_choices.clear();
        })?;
        let staging = self.inner.storage.create_staging(id, &job.draft.output)?;
        self.mutate(id, Some((generation, run)), true, |job| {
            job.staging_path = Some(staging.clone())
        })?;
        let request = ConversionRequest {
            bundle,
            options: if matches!(job.draft.options.encoding, EncodingChoice::Auto)
                && !metadata.source_encoding.is_empty()
            {
                ReadOptions {
                    encoding: EncodingChoice::Manual(metadata.source_encoding.clone()),
                }
            } else {
                job.draft.options.clone()
            },
            metadata_override: job.draft.metadata_override.clone(),
            staging_dir: staging,
        };
        let core_context = context.clone();
        let artifacts =
            tokio::task::spawn_blocking(move || convert_to_staging(&request, &core_context))
                .await
                .map_err(|_| AppError::new("worker_failed", "Не удалось завершить конвертацию"))?;
        if let Some(error) = callback_error.lock().map_err(|_| lock_error())?.take() {
            return Err(error);
        }
        let artifacts = artifacts?;
        super::report::augment(&artifacts, &audits, &job.draft.options, &context)?;
        self.mutate(id, Some((generation, run)), true, |job| {
            job.ready = Some(artifacts.clone());
            job.snapshot.state = JobState::Publishing;
        })?;
        self.publish_result(id, generation, run, artifacts, &job.draft.output, &context)
            .await
    }
    fn run_context(
        &self,
        id: &str,
        generation: u64,
        run: &str,
        controls: &Controls,
    ) -> AppResult<(RunContext, Arc<Mutex<Option<AppError>>>)> {
        let context = RunContext::new(self.inner.store.root.join("scratch").join(id));
        let mut context = context;
        context.control = controls.conversion.clone();
        let app = self.clone();
        let (job_id, run_id) = (id.to_owned(), run.to_owned());
        let callback_error = Arc::new(Mutex::new(None));
        let failure = callback_error.clone();
        let stop = controls.conversion.clone();
        context.progress = Arc::new(move |event| {
            if let Err(error) = app.accept_core_event(&job_id, generation, &run_id, event) {
                if let Ok(mut failure) = failure.lock() {
                    *failure = Some(error);
                }
                stop.cancel();
            }
        });
        fs::create_dir_all(&context.scratch_dir)?;
        Ok((context, callback_error))
    }
    async fn publish_result(
        &self,
        id: &str,
        generation: u64,
        run: &str,
        artifacts: ConvertedArtifacts,
        output: &OutputChoice,
        context: &RunContext,
    ) -> AppResult<()> {
        let published = self
            .inner
            .storage
            .publish(artifacts, output, context)
            .await?;
        let files = ["dictionary.mdx", "dictionary.mdd", "conversion-report.json"]
            .into_iter()
            .map(|name| {
                Ok(ResultFile {
                    name: name.into(),
                    bytes: fs::metadata(published.directory.join(name))?.len(),
                })
            })
            .collect::<AppResult<Vec<_>>>()?;
        let info = ResultInfo {
            id: published.id.clone(),
            files,
            summary: published.summary.clone(),
            can_open: true,
            can_export: true,
        };
        self.mutate(id, Some((generation, run)), true, |job| {
            job.snapshot.state = if info.summary.warnings > 0 {
                JobState::SucceededWithWarnings
            } else {
                JobState::Succeeded
            };
            job.snapshot.conversion.status = ConversionStatus::Succeeded;
            job.snapshot.result = Some(info);
            job.published = Some(published);
            job.ready = None;
            job.staging_path = None;
        })?;
        Ok(())
    }
    fn stopped_download(
        &self,
        id: &str,
        generation: u64,
        run: &str,
        controls: &Controls,
    ) -> AppResult<()> {
        self.mutate(id, Some((generation, run)), true, |job| {
            job.snapshot.state = if controls.download.state() == TransferState::Cancelled {
                JobState::Cancelled
            } else if job.snapshot.download.kind == DownloadKind::Remote {
                JobState::DownloadPaused
            } else {
                JobState::Interrupted
            };
        })?;
        Ok(())
    }
}
