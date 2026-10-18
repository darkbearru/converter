use converter_app::JobManager;
use tauri::{AppHandle, Emitter};
pub fn forward(app: AppHandle, manager: JobManager) {
    let mut receiver = manager.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    if app.emit("job:event", event).is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    if let Ok(snapshots) = manager.list() {
                        for snapshot in snapshots {
                            let _ = app.emit("job:event", converter_app::JobEvent { snapshot });
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}
