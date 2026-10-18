mod commands;
mod events;
mod storage;
use converter_app::{JobManager, storage::desktop::DesktopStorage};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tauri::{Emitter, Manager};

pub struct NativeState {
    manager: JobManager,
    storage: Arc<DesktopStorage>,
}

fn shutdown(app: tauri::AppHandle, closing: &AtomicBool) {
    if closing.swap(true, Ordering::SeqCst) {
        return;
    }
    let manager = app.state::<NativeState>().manager.clone();
    tauri::async_runtime::spawn(async move {
        let code = if manager.shutdown().await.is_ok() {
            0
        } else {
            1
        };
        app.exit(code);
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let closing = Arc::new(AtomicBool::new(false));
    let window_closing = closing.clone();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let root = app.path().app_data_dir()?;
            let storage = Arc::new(DesktopStorage::open(root.clone())?);
            let manager = JobManager::open(root, storage.clone())?;
            events::forward(app.handle().clone(), manager.clone());
            app.manage(NativeState { manager, storage });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_formats,
            commands::get_jobs,
            commands::pick_sources,
            commands::pick_output,
            commands::create_job,
            commands::start_job,
            commands::pause_download,
            commands::resume_download,
            commands::cancel_job,
            commands::delete_job,
            commands::update_job,
            commands::resolve_additional_dictionaries,
            commands::read_diagnostics,
            commands::export_result,
            commands::open_result,
            commands::get_result_path
        ])
        .on_window_event(move |window, event| {
            if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                let app = window.app_handle().clone();
                let manager = app.state::<NativeState>().manager.clone();
                let paths = paths.clone();
                tauri::async_runtime::spawn(async move {
                    let event = match manager
                        .import(converter_app::storage::PlatformSelection::LocalPaths(paths))
                        .await
                    {
                        Ok(files) => converter_app::SourceDropEvent::Files { files },
                        Err(error) => converter_app::SourceDropEvent::Error { error },
                    };
                    let _ = app.emit("source:drop", event);
                });
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                shutdown(window.app_handle().clone(), &window_closing);
            }
        })
        .build(tauri::generate_context!())
        .expect("Не удалось запустить Converter");
    app.run(move |app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event
            && !closing.load(Ordering::SeqCst)
        {
            api.prevent_exit();
            shutdown(app.clone(), &closing);
        }
    });
}
