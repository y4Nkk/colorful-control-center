use crate::hal::oem_owner::{self, FnKeyIdentity, FnKeyProcess};
use tauri::{AppHandle, Emitter};

#[tauri::command]
pub(crate) async fn read_fn_key_close_target() -> Result<Option<FnKeyIdentity>, String> {
    crate::shutdown::blocking(|| {
        FnKeyProcess::blocking().map(|process| process.map(|process| process.identity()))
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
pub(crate) async fn close_fn_key(app: AppHandle, expected: FnKeyIdentity) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        if let Some(process) = FnKeyProcess::blocking()? {
            process.close(expected)?;
        }
        oem_owner::ensure_owner_absent()?;
        if let Err(error) = app.emit_to("main", "hardware-controls-changed", ()) {
            log::error!("[oem_owner] 刷新控制状态：{error:#}");
        }
        Ok(())
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error: anyhow::Error| format!("{error:#}"))
}
