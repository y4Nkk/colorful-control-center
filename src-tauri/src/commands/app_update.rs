use crate::app_update::{self, CandidateInfo, Progress};
use tauri::{AppHandle, ipc::Channel};

#[tauri::command]
pub(crate) async fn check_application_update(
    app: AppHandle,
) -> Result<Option<CandidateInfo>, String> {
    app_update::check(&app)
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
pub(crate) async fn download_application_update(
    app: AppHandle,
    id: u32,
    progress: Channel<Progress>,
) -> Result<bool, String> {
    app_update::download(&app, id, progress)
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
pub(crate) fn cancel_application_update(app: AppHandle, id: u32) -> bool {
    app_update::cancel(&app, id)
}
