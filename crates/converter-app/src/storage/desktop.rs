//! Desktop использует только назначения, выданные нативным picker.
use super::*;
use converter_core::pipeline::publish_local;
use std::{
    collections::BTreeSet,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct DesktopStorage {
    root: PathBuf,
    library: PathBuf,
    grants: Arc<Mutex<BTreeSet<PathBuf>>>,
}
impl DesktopStorage {
    pub fn open(root: PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(&root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))?;
        }
        let root = root.canonicalize()?;
        let library = root.join("results");
        std::fs::create_dir_all(&library)?;
        let grants_path = root.join("destinations.json");
        let grants = if grants_path.exists() {
            serde_json::from_reader(File::open(grants_path)?)?
        } else {
            BTreeSet::new()
        };
        Ok(Self {
            root,
            library,
            grants: Arc::new(Mutex::new(grants)),
        })
    }
    pub fn grant_output(&self, path: &Path) -> AppResult<OutputChoice> {
        let path = path.canonicalize().map_err(|_| unavailable())?;
        if !path.is_dir() {
            return Err(unavailable());
        }
        let mut grants = self.grants.lock().map_err(|_| unavailable())?;
        let mut next = grants.clone();
        next.insert(path.clone());
        atomicwrites::AtomicFile::new(
            self.root.join("destinations.json"),
            atomicwrites::AllowOverwrite,
        )
        .write(|file| {
            serde_json::to_writer(&mut *file, &next)?;
            file.sync_all()
        })
        .map_err(|_| AppError::new("storage_error", "Не удалось сохранить выбранную папку"))?;
        *grants = next;
        Ok(OutputChoice::DesktopFolder(path))
    }
    fn destination(&self, output: &OutputChoice) -> AppResult<PathBuf> {
        match output {
            OutputChoice::AppLibrary => Ok(self.library.clone()),
            OutputChoice::DesktopFolder(path) => {
                let canonical = path.canonicalize().map_err(|_| unavailable())?;
                if canonical != *path
                    || !canonical.is_dir()
                    || !self
                        .grants
                        .lock()
                        .map_err(|_| unavailable())?
                        .contains(&canonical)
                {
                    return Err(unavailable());
                }
                Ok(canonical)
            }
        }
    }
    pub fn validate_output(&self, output: &OutputChoice) -> AppResult<()> {
        self.destination(output).map(|_| ())
    }
    fn check_result(&self, directory: &Path) -> AppResult<[PathBuf; 3]> {
        if std::fs::symlink_metadata(directory)?
            .file_type()
            .is_symlink()
        {
            return Err(invalid_result());
        }
        let directory = directory.canonicalize()?;
        let parent = directory.parent().ok_or_else(invalid_result)?;
        if parent != self.library
            && !self
                .grants
                .lock()
                .map_err(|_| unavailable())?
                .contains(parent)
        {
            return Err(invalid_result());
        }
        let files = ["dictionary.mdx", "dictionary.mdd", "conversion-report.json"]
            .map(|name| directory.join(name));
        if std::fs::read_dir(&directory)?.count() != files.len() {
            return Err(invalid_result());
        }
        for file in &files {
            let meta = std::fs::symlink_metadata(file)?;
            if !meta.file_type().is_file()
                || file.canonicalize()?.parent() != Some(directory.as_path())
            {
                return Err(invalid_result());
            }
        }
        Ok(files)
    }
    pub fn checked_result_directory(&self, directory: &Path) -> AppResult<PathBuf> {
        self.check_result(directory)?;
        Ok(directory.canonicalize()?)
    }
}
fn unavailable() -> AppError {
    AppError::new(
        "output_unavailable",
        "Папка результата недоступна. Выберите её заново.",
    )
}
fn invalid_result() -> AppError {
    AppError::new(
        "invalid_result",
        "Файлы результата изменились или недоступны",
    )
}
fn join_error(_: tokio::task::JoinError) -> AppError {
    AppError::new("storage_error", "Операция с файлами прервана")
}
fn report_name(path: &Path, ctx: &RunContext) -> AppResult<String> {
    #[derive(serde::Deserialize)]
    struct Report {
        metadata: converter_core::model::DictionaryMetadata,
    }
    struct CheckedReader<'a> {
        file: File,
        ctx: &'a RunContext,
    }
    impl Read for CheckedReader<'_> {
        fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
            self.ctx.control.check().map_err(std::io::Error::other)?;
            self.file.read(bytes)
        }
    }
    let report: std::result::Result<Report, _> =
        serde_json::from_reader(std::io::BufReader::new(CheckedReader {
            file: File::open(path)?,
            ctx,
        }));
    ctx.control.check()?;
    let title = report?.metadata.title;
    Ok(if title.trim().is_empty() {
        "dictionary".into()
    } else {
        title
    })
}

#[async_trait::async_trait]
impl PlatformStorage for DesktopStorage {
    async fn import(
        &self,
        selection: PlatformSelection,
        ctx: &RunContext,
    ) -> AppResult<Vec<PreparedFile>> {
        let PlatformSelection::LocalPaths(paths) = selection else {
            return Err(AppError::new(
                "unsupported_selection",
                "Выберите локальные файлы",
            ));
        };
        let ctx = ctx.clone();
        tokio::task::spawn_blocking(move || {
            paths
                .into_iter()
                .map(|path| {
                    ctx.control.check()?;
                    let path = path.canonicalize()?;
                    let metadata = std::fs::metadata(&path)?;
                    if !metadata.is_file() && !metadata.is_dir() {
                        return Err(AppError::new(
                            "invalid_source",
                            "Нужен обычный файл или папка",
                        ));
                    }
                    let display_name = path
                        .file_name()
                        .ok_or_else(|| {
                            AppError::new("invalid_source", "Выберите файл или папку ресурсов")
                        })?
                        .to_string_lossy()
                        .into_owned();
                    Ok(PreparedFile {
                        id: uuid::Uuid::new_v4().to_string(),
                        display_name,
                        path,
                    })
                })
                .collect()
        })
        .await
        .map_err(join_error)?
    }
    fn create_staging(&self, job_id: &str, output: &OutputChoice) -> AppResult<PathBuf> {
        if job_id.is_empty()
            || job_id.len() > 128
            || !job_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(AppError::new("invalid_id", "Некорректное задание"));
        }
        Ok(self
            .destination(output)?
            .join(format!(".converter-{job_id}-{}", uuid::Uuid::new_v4())))
    }
    async fn publish(
        &self,
        artifacts: ConvertedArtifacts,
        output: &OutputChoice,
        ctx: &RunContext,
    ) -> AppResult<PublishedResult> {
        let destination = self.destination(output)?;
        let ctx = ctx.clone();
        tokio::task::spawn_blocking(move || {
            let summary = artifacts.summary.clone();
            let name = report_name(&artifacts.report, &ctx)?;
            let directory = publish_local(artifacts, &destination, &name, &ctx)?;
            Ok(PublishedResult {
                id: uuid::Uuid::new_v4().to_string(),
                directory,
                summary,
            })
        })
        .await
        .map_err(join_error)?
    }
    async fn export(
        &self,
        result: &PublishedResult,
        choice: ExportChoice,
        ctx: &RunContext,
    ) -> AppResult<ExportSnapshot> {
        let ExportChoice::Folder(path) = choice else {
            return Err(AppError::new(
                "unsupported_destination",
                "Выберите папку для копии",
            ));
        };
        let destination = self.destination(&OutputChoice::DesktopFolder(path))?;
        let files = self.check_result(&result.directory)?;
        let summary = result.summary.clone();
        let ctx = ctx.clone();
        tokio::task::spawn_blocking(move || {
            ctx.control.check()?;
            let name = report_name(&files[2], &ctx)?;
            let staging = tempfile::Builder::new()
                .prefix(".converter-export-")
                .tempdir_in(&destination)?;
            for source in &files {
                ctx.control.check()?;
                let name = source.file_name().ok_or_else(invalid_result)?;
                let mut input = File::open(source)?;
                let mut output = File::create_new(staging.path().join(name))?;
                let mut buffer = vec![0; 64 * 1024];
                loop {
                    ctx.control.check()?;
                    let count = input.read(&mut buffer)?;
                    if count == 0 {
                        break;
                    }
                    output.write_all(&buffer[..count])?;
                }
                output.sync_all()?;
            }
            let artifacts = ConvertedArtifacts {
                directory: staging.path().into(),
                mdx: staging.path().join("dictionary.mdx"),
                mdd: staging.path().join("dictionary.mdd"),
                report: staging.path().join("conversion-report.json"),
                summary,
            };
            let directory = publish_local(artifacts, &destination, &name, &ctx)?;
            Ok(ExportSnapshot {
                status: ExportStatus::Succeeded,
                saved_files: files
                    .iter()
                    .filter_map(|file| file.file_name())
                    .map(|name| directory.join(name).to_string_lossy().into_owned())
                    .collect(),
                error: None,
            })
        })
        .await
        .map_err(join_error)?
    }
}
