#![allow(dead_code)]
use converter_app::{storage::*, *};
use converter_core::{model::*, pipeline::publish_local};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
pub struct TestStorage {
    pub root: PathBuf,
    pub invocations: AtomicUsize,
    active: AtomicUsize,
    pub peak: AtomicUsize,
    pub fail_publish: AtomicBool,
    pub block_export: AtomicBool,
    pub export_started: tokio::sync::Notify,
    pub export_release: tokio::sync::Notify,
}
impl TestStorage {
    pub fn new(root: &Path) -> Arc<Self> {
        Arc::new(Self {
            root: root.into(),
            invocations: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            fail_publish: AtomicBool::new(false),
            block_export: AtomicBool::new(false),
            export_started: tokio::sync::Notify::new(),
            export_release: tokio::sync::Notify::new(),
        })
    }
}
#[async_trait::async_trait]
impl PlatformStorage for TestStorage {
    async fn import(
        &self,
        selection: PlatformSelection,
        _ctx: &RunContext,
    ) -> AppResult<Vec<PreparedFile>> {
        let PlatformSelection::LocalPaths(paths) = selection else {
            return Err(AppError::new(
                "unsupported_selection",
                "Выберите локальные файлы",
            ));
        };
        paths
            .into_iter()
            .map(|path| {
                let display_name = path
                    .file_name()
                    .ok_or_else(|| AppError::new("invalid_path", "Нужно имя файла"))?
                    .to_string_lossy()
                    .to_string();
                Ok(PreparedFile {
                    id: uuid::Uuid::new_v4().to_string(),
                    path,
                    display_name,
                })
            })
            .collect()
    }
    fn create_staging(&self, id: &str, output: &OutputChoice) -> AppResult<PathBuf> {
        self.invocations.fetch_add(1, Ordering::SeqCst);
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(active, Ordering::SeqCst);
        let root = match output {
            OutputChoice::DesktopFolder(path) => path.clone(),
            _ => self.root.join("results"),
        };
        std::fs::create_dir_all(&root)?;
        Ok(root.join(format!(".converter-{id}-{}", uuid::Uuid::new_v4())))
    }
    async fn publish(
        &self,
        artifacts: ConvertedArtifacts,
        output: &OutputChoice,
        ctx: &RunContext,
    ) -> AppResult<PublishedResult> {
        if self.fail_publish.swap(false, Ordering::SeqCst) {
            return Err(AppError::new(
                "output_unavailable",
                "Выбранная папка недоступна",
            ));
        }
        let root = match output {
            OutputChoice::DesktopFolder(path) => path.clone(),
            _ => self.root.join("results"),
        };
        let summary = artifacts.summary.clone();
        let directory = publish_local(artifacts, &root, "Fixture", ctx)?;
        self.active.fetch_sub(1, Ordering::SeqCst);
        Ok(PublishedResult {
            id: uuid::Uuid::new_v4().to_string(),
            directory,
            summary,
        })
    }
    async fn export(
        &self,
        _result: &PublishedResult,
        _choice: ExportChoice,
        _ctx: &RunContext,
    ) -> AppResult<ExportSnapshot> {
        if self.block_export.load(Ordering::SeqCst) {
            self.export_started.notify_one();
            self.export_release.notified().await;
        }
        Err(AppError::new(
            "not_supported",
            "Экспорт проверяется адаптером desktop",
        ))
    }
}
pub fn draft(source: SourceInput) -> JobDraft {
    JobDraft {
        source,
        resources: vec![],
        output: OutputChoice::AppLibrary,
        options: ReadOptions::default(),
        metadata_override: None,
        auto_start: false,
        prepare_only: false,
    }
}
pub fn remote(url: String) -> SourceInput {
    SourceInput::Remote {
        url,
        expected_sha256: None,
    }
}
pub async fn local(manager: &JobManager, root: &Path, name: &str) -> JobDraft {
    let path = root.join(name);
    std::fs::write(&path, b"#NAME \"Fixture\"\nalpha\n\tBody\n").expect("fixture");
    let imported = manager
        .import(PlatformSelection::LocalPaths(vec![path]))
        .await
        .expect("import");
    draft(SourceInput::Local {
        import_id: imported[0].import_id.clone(),
    })
}
pub async fn wait(manager: &JobManager, id: &str, expected: &[JobState]) -> JobSnapshot {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let snapshot = manager
                .list()
                .expect("list")
                .into_iter()
                .find(|s| s.id == id)
                .expect("job");
            if expected.contains(&snapshot.state) {
                return snapshot;
            }
            if matches!(snapshot.state, JobState::Failed | JobState::DownloadFailed) {
                panic!("unexpected failure: {:?}", snapshot.error);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("job timeout: {:?}", manager.list()))
}
