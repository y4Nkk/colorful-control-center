use tauri::{AppHandle, Manager};

#[tauri::command]
pub(crate) fn read_osd(app: AppHandle) -> Result<crate::osd::Snapshot, String> {
    app.state::<crate::osd::Controller>()
        .snapshot(&app)
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) fn preview_osd(app: AppHandle) -> Result<(), String> {
    app.state::<crate::osd::Controller>()
        .preview(&app)
        .map_err(|e| format!("{e:#}"))
}
