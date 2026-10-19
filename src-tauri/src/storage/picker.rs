use crate::{localization::native_text, preferences::Locale};
use converter_app::{AppError, AppResult};
use std::path::PathBuf;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

pub async fn sources(app: AppHandle, purpose: String, locale: Locale) -> AppResult<Vec<PathBuf>> {
    if !["dictionaries", "resources", "resource_directory"].contains(&purpose.as_str()) {
        return Err(
            AppError::new("invalid_selection", "Неизвестный тип источника").with_message(
                "messages.invalid_selection",
                converter_core::model::message_args([]),
            ),
        );
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
                .set_title(native_text(locale, "native.pick_dictionaries"))
                .add_filter(
                    native_text(locale, "native.dictionary_filter"),
                    &extension_refs,
                )
                .blocking_pick_files()
                .unwrap_or_default(),
            "resource_directory" => builder
                .set_title(native_text(locale, "native.pick_resource_directory"))
                .blocking_pick_folder()
                .into_iter()
                .collect(),
            _ => builder
                .set_title(native_text(locale, "native.pick_resources"))
                .blocking_pick_files()
                .unwrap_or_default(),
        };
        files
            .into_iter()
            .map(|path| {
                path.into_path().map_err(|_| {
                    AppError::new("invalid_selection", "Выберите локальный файл").with_message(
                        "messages.invalid_selection_2",
                        converter_core::model::message_args([]),
                    )
                })
            })
            .collect()
    })
    .await
    .map_err(|_| {
        AppError::new("picker_error", "Не удалось открыть выбор файлов").with_message(
            "messages.picker_error",
            converter_core::model::message_args([]),
        )
    })?
}
pub async fn output(app: AppHandle, locale: Locale) -> AppResult<Option<PathBuf>> {
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title(native_text(locale, "native.pick_output"))
            .blocking_pick_folder()
            .map(|path| {
                path.into_path().map_err(|_| {
                    AppError::new("invalid_selection", "Выберите локальную папку").with_message(
                        "messages.invalid_selection_3",
                        converter_core::model::message_args([]),
                    )
                })
            })
            .transpose()
    })
    .await
    .map_err(|_| {
        AppError::new("picker_error", "Не удалось открыть выбор папки").with_message(
            "messages.picker_error_2",
            converter_core::model::message_args([]),
        )
    })?
}
