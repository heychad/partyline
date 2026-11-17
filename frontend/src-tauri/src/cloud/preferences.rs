use super::library::{self, model::WorkspacePreferences};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use super::library::model::{OutlineState, OutlineChange};
static LOCK: Mutex<()> = Mutex::const_new(());
pub fn load(app: &AppHandle) -> WorkspacePreferences {
    app.path()
        .app_data_dir()
        .ok()
        .and_then(|p| std::fs::read(p.join("workspace-preferences.json")).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .filter(|p: &WorkspacePreferences| p.validate().is_ok())
        .unwrap_or_default()
}
#[tauri::command]
pub fn cloud_preferences(app: AppHandle) -> WorkspacePreferences {
    load(&app)
}
#[tauri::command]
pub async fn cloud_preferences_update(
    app: AppHandle,
    patch: Value,
) -> Result<WorkspacePreferences, String> {
    let _guard = LOCK.lock().await;
    let next = load(&app).patched(patch)?;
    let folder = app
        .path()
        .app_data_dir()
        .map_err(|_| "Could not locate settings.")?;
    library::atomic_json(&folder.join("workspace-preferences.json"), &next)?;
    let _ = app.emit("cloud-preferences", &next);
    Ok(next)
}

#[tauri::command]
pub async fn cloud_outline(app: AppHandle, id: String) -> Result<OutlineState, String> {
    let shared = super::get(&app, &id).await?;
    let path = shared.folder.join("outline-state.json");
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| "Could not read outline settings.".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(OutlineState::from_legacy(&load(&app).collapsed_notes)),
        Err(_) => Err("Could not read outline settings.".into()),
    }
}

#[tauri::command]
pub async fn cloud_outline_update(app: AppHandle, id: String, change: OutlineChange) -> Result<OutlineState, String> {
    let _guard = LOCK.lock().await;
    let shared = super::get(&app, &id).await?;
    let next = cloud_outline(app.clone(), id.clone()).await?.changed(change)?;
    library::atomic_json(&shared.folder.join("outline-state.json"), &next)?;
    let _ = app.emit("cloud-outline", serde_json::json!({"id":id,"state":next}));
    Ok(next)
}
