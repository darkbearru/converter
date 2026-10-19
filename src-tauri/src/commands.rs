use crate::{NativeState, storage::picker};
use converter_app::{
    storage::{ExportChoice, PlatformSelection},
    *,
};
use converter_core::formats::{FormatDescriptor, FormatRegistry};
use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub fn get_app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> AppResult<crate::updates::UpdateInfo> {
    crate::updates::check(&app.package_info().version.to_string()).await
}

#[tauri::command]
pub fn open_release_page(app: AppHandle) -> AppResult<()> {
    app.opener()
        .open_url(crate::updates::RELEASE_PAGE, None::<&str>)
        .map_err(|_| AppError::new("open_failed", "Не удалось открыть страницу релиза"))
}

#[tauri::command]
pub fn read_clipboard_text(app: AppHandle) -> AppResult<String> {
    app.clipboard().read_text().map_err(|_| {
        AppError::new(
            "clipboard_unavailable",
            "Не удалось прочитать текст из буфера обмена. Вставьте ссылку с клавиатуры.",
        )
    })
}

#[tauri::command]
pub fn open_support_page(app: AppHandle) -> AppResult<()> {
    app.opener()
        .open_url("https://pay.cloudtips.ru/p/7e782291", None::<&str>)
        .map_err(|_| AppError::new("open_failed", "Не удалось открыть страницу поддержки"))
}

#[tauri::command]
pub async fn open_issue(
    app: AppHandle,
    state: State<'_, NativeState>,
    id: Option<String>,
) -> AppResult<()> {
    let manager = state.manager.clone();
    let version = app.package_info().version.to_string();
    let url = tauri::async_runtime::spawn_blocking(move || {
        let job = id
            .as_deref()
            .map(|id| {
                manager
                    .list()?
                    .into_iter()
                    .find(|job| job.id == id)
                    .ok_or_else(|| AppError::new("job_not_found", "Задание не найдено"))
            })
            .transpose()?;
        // Ошибка чтения журнала не должна мешать сообщить о самой проблеме.
        let diagnostics = job
            .as_ref()
            .and_then(|job| manager.diagnostics(&job.id, 0, 20).ok())
            .map(|page| page.items)
            .unwrap_or_default();
        crate::issue_report::issue_url(
            &version,
            std::env::consts::OS,
            std::env::consts::ARCH,
            job.as_ref(),
            &diagnostics,
        )
    })
    .await
    .map_err(|_| {
        AppError::new(
            "issue_report_failed",
            "Не удалось подготовить сообщение. Повторите попытку.",
        )
    })??;
    app.opener().open_url(url, None::<&str>).map_err(|_| {
        AppError::new(
            "open_failed",
            "Не удалось открыть браузер. Повторите попытку.",
        )
    })
}

fn snapshot(state: &NativeState, id: &str) -> AppResult<JobSnapshot> {
    state
        .manager
        .list()?
        .into_iter()
        .find(|j| j.id == id)
        .ok_or_else(|| AppError::new("job_not_found", "Задание не найдено"))
}
#[tauri::command]
pub fn list_formats() -> Vec<FormatDescriptor> {
    FormatRegistry::builtins().descriptors()
}
#[tauri::command]
pub fn get_jobs(state: State<'_, NativeState>) -> AppResult<Vec<JobSnapshot>> {
    state.manager.list()
}
#[tauri::command]
pub async fn pick_sources(
    app: AppHandle,
    state: State<'_, NativeState>,
    purpose: String,
) -> AppResult<Vec<ImportedSourceInfo>> {
    let paths = picker::sources(app, purpose).await?;
    state
        .manager
        .import(PlatformSelection::LocalPaths(paths))
        .await
}
#[tauri::command]
pub async fn pick_output(
    app: AppHandle,
    state: State<'_, NativeState>,
) -> AppResult<Option<OutputChoice>> {
    picker::output(app)
        .await?
        .map(|path| state.storage.grant_output(&path))
        .transpose()
}
#[tauri::command]
pub async fn create_job(state: State<'_, NativeState>, draft: JobDraft) -> AppResult<JobSnapshot> {
    state.storage.validate_output(&draft.output)?;
    state.manager.create(draft).await
}
#[tauri::command]
pub async fn start_job(state: State<'_, NativeState>, id: String) -> AppResult<JobSnapshot> {
    state.manager.start(&id).await
}
#[tauri::command]
pub async fn pause_download(state: State<'_, NativeState>, id: String) -> AppResult<JobSnapshot> {
    state.manager.pause_download(&id).await
}
#[tauri::command]
pub async fn resume_download(state: State<'_, NativeState>, id: String) -> AppResult<JobSnapshot> {
    state.manager.resume_download(&id).await
}
#[tauri::command]
pub async fn cancel_job(state: State<'_, NativeState>, id: String) -> AppResult<JobSnapshot> {
    state.manager.cancel(&id).await
}
#[tauri::command]
pub async fn delete_job(state: State<'_, NativeState>, id: String) -> AppResult<()> {
    state.manager.delete(&id).await
}
#[tauri::command]
pub async fn update_job(
    state: State<'_, NativeState>,
    id: String,
    update: JobOptionsUpdate,
) -> AppResult<JobSnapshot> {
    if let Some(output) = &update.output {
        state.storage.validate_output(output)?;
    }
    state.manager.update(&id, update).await
}
#[tauri::command]
pub async fn resolve_additional_dictionaries(
    state: State<'_, NativeState>,
    job_id: String,
    offer_id: String,
    convert: bool,
) -> AppResult<Vec<JobSnapshot>> {
    state
        .manager
        .resolve_additional(&job_id, &offer_id, convert)
        .await
}
#[tauri::command]
pub fn read_diagnostics(
    state: State<'_, NativeState>,
    id: String,
    offset: u64,
    limit: u32,
) -> AppResult<DiagnosticPage> {
    state.manager.diagnostics(&id, offset, limit)
}
#[tauri::command]
pub async fn export_result(
    app: AppHandle,
    state: State<'_, NativeState>,
    id: String,
) -> AppResult<JobSnapshot> {
    if let Some(path) = picker::output(app).await? {
        let OutputChoice::DesktopFolder(path) = state.storage.grant_output(&path)? else {
            unreachable!()
        };
        state
            .manager
            .export(&id, ExportChoice::Folder(path))
            .await?;
    }
    snapshot(&state, &id)
}
#[tauri::command]
pub fn open_result(app: AppHandle, state: State<'_, NativeState>, id: String) -> AppResult<()> {
    let path = state
        .storage
        .checked_result_directory(&state.manager.result_directory(&id)?)?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|_| AppError::new("open_failed", "Не удалось открыть папку результата"))
}
#[tauri::command]
pub fn get_result_path(state: State<'_, NativeState>, id: String) -> AppResult<String> {
    Ok(state
        .storage
        .checked_result_directory(&state.manager.result_directory(&id)?)?
        .to_string_lossy()
        .into_owned())
}
