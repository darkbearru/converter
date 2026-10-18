use converter_app::{AppError, AppResult};
use std::path::PathBuf;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

pub async fn sources(app: AppHandle, purpose: String) -> AppResult<Vec<PathBuf>> {
    if !["dictionaries", "resources", "resource_directory"].contains(&purpose.as_str()) {
        return Err(AppError::new(
            "invalid_selection",
            "Неизвестный тип источника",
        ));
    }
    tauri::async_runtime::spawn_blocking(move || {
        let builder = app.dialog().file();
        let mut extensions: Vec<String> = converter_core::formats::FormatRegistry::builtins()
            .descriptors()
            .into_iter()
            .flat_map(|format| format.extensions)
            .map(|extension| {
                extension
                    .rsplit('.')
                    .next()
                    .unwrap_or(&extension)
                    .to_owned()
            })
            .collect();
        extensions.push("zip".into());
        extensions.sort();
        extensions.dedup();
        let extension_refs: Vec<&str> = extensions.iter().map(String::as_str).collect();
        let files = match purpose.as_str() {
            "dictionaries" => builder
                .set_title("Выберите словари")
                .add_filter("Словари и архивы", &extension_refs)
                .blocking_pick_files()
                .unwrap_or_default(),
            "resource_directory" => builder
                .set_title("Выберите папку ресурсов")
                .blocking_pick_folder()
                .into_iter()
                .collect(),
            _ => builder
                .set_title("Выберите ресурсы или ZIP")
                .blocking_pick_files()
                .unwrap_or_default(),
        };
        files
            .into_iter()
            .map(|path| {
                path.into_path()
                    .map_err(|_| AppError::new("invalid_selection", "Выберите локальный файл"))
            })
            .collect()
    })
    .await
    .map_err(|_| AppError::new("picker_error", "Не удалось открыть выбор файлов"))?
}
pub async fn output(app: AppHandle) -> AppResult<Option<PathBuf>> {
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Выберите папку результата")
            .blocking_pick_folder()
            .map(|path| {
                path.into_path()
                    .map_err(|_| AppError::new("invalid_selection", "Выберите локальную папку"))
            })
            .transpose()
    })
    .await
    .map_err(|_| AppError::new("picker_error", "Не удалось открыть выбор папки"))?
}
