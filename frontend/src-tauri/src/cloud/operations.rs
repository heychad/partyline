use super::{get, library::{self, model::Meeting, Edit}, root};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

pub async fn all(app: &AppHandle) -> Result<Vec<Meeting>, String> {
    let folder = root(app)?;
    tokio::fs::create_dir_all(&folder).await.map_err(|_| "Could not open the meetings folder.")?;
    let mut entries = tokio::fs::read_dir(folder).await.map_err(|_| "Could not list meetings.")?;
    let mut meetings = Vec::new();
    while let Some(entry) = entries.next_entry().await.map_err(|_| "Could not read meeting history.")? {
        if let Ok(shared) = get(app, &entry.file_name().to_string_lossy()).await { meetings.push(shared.meeting.lock().await.clone()); }
    }
    meetings.sort_by(|a,b| b.created_at.cmp(&a.created_at));
    Ok(meetings)
}

#[tauri::command]
pub async fn cloud_library(app: AppHandle, trash: bool, query: String) -> Result<Value, String> {
    if query.chars().count() > 500 { return Err("Use a shorter search.".into()); }
    let query = query.trim().to_lowercase();
    let items: Vec<_> = all(&app).await?.into_iter().filter(|m| m.deleted_at.is_some() == trash)
        .filter(|m| query.is_empty() || format!("{} {} {}",m.title,m.participants.join(" "),m.transcript()).to_lowercase().contains(&query))
        .map(|m| json!({"id":m.id,"title":m.title,"created_at":m.created_at,"status":m.status,"duration":m.duration,"deleted_at":m.deleted_at,"edit_version":m.edit_version})).collect();
    Ok(json!({"items":items}))
}

#[tauri::command]
pub async fn cloud_edit(app: AppHandle, id: String, expected_version: u64, edit: Edit) -> Result<Meeting, String> {
    let s = get(&app, &id).await?;
    let _notes = s.notes_lock.try_lock().map_err(|_| "Wait for notes to finish, then save your edit.")?;
    let _write = s.write_lock.lock().await;
    let mut current = s.meeting.lock().await;
    let folder = s.folder.clone(); let snapshot = current.clone();
    let next = tokio::task::spawn_blocking(move || library::save_edit(&folder, &snapshot, expected_version, &edit))
        .await.map_err(|_| "The edit could not finish. Reload the meeting.")??;
    *current = next.clone();
    let _ = app.emit("cloud-meeting", &next);
    let _ = app.emit("cloud-library", json!({"id":id,"edit_version":next.edit_version}));
    let backup_app=app.clone();let backup_id=id.clone();
    if next.deleted_at.is_none() { tauri::async_runtime::spawn(async move { super::backup::automatic(backup_app,backup_id).await; }); }
    Ok(next)
}

#[tauri::command]
pub async fn cloud_search(app: AppHandle, query: String, limit: usize) -> Result<Value, String> {
    if query.chars().count() > 500 || limit == 0 || limit > 200 { return Err("Use a search under 500 characters and a limit of 1–200.".into()); }
    let meetings = all(&app).await?;
    Ok(json!({"items":library::search(&meetings,&query,limit)}))
}

#[tauri::command]
pub async fn cloud_audio(app: AppHandle, id: String, channel: String) -> Result<Value, String> {
    if !["me", "remote", "both"].contains(&channel.as_str()) { return Err("Choose both, me, or remote audio.".into()); }
    let s = get(&app, &id).await?;
    let _write = s.write_lock.lock().await;
    let m = s.meeting.lock().await.clone();
    library::idle(&m)?;
    if m.deleted_at.is_some() { return Err("Restore this recording before playing it.".into()); }
    let folder = s.folder.clone();
    let export = folder.join("exports");
    tokio::fs::create_dir_all(&export).await.map_err(|_| "Could not create the audio export folder.")?;
    let output = export.join(format!("audio-{}-{}.wav",m.edit_version,channel));
    if !output.is_file() {
        let snapshot=m.clone();let target=output.clone();let source=channel.clone();
        tokio::task::spawn_blocking(move || library::render_audio(&snapshot,&folder,&source,&target))
            .await.map_err(|_| "Could not prepare playback audio.")??;
    }
    Ok(json!({"meeting_id":id,"path":output,"channel":channel,"start":m.trim_start,"end":m.trim_end.unwrap_or(m.duration),"duration":m.trim_end.unwrap_or(m.duration)-m.trim_start,"edit_version":m.edit_version,"redacted":!m.redactions.is_empty()}))
}

pub async fn copy(text: &str) -> Result<(), String> {
    let mut child=tokio::process::Command::new("/usr/bin/pbcopy").stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().map_err(|_| "Could not open the clipboard.")?;
    let mut input=child.stdin.take().ok_or("Clipboard input unavailable.")?;
    input.write_all(text.as_bytes()).await.map_err(|_| "Could not copy to the clipboard.")?;drop(input);
    if !child.wait().await.map_err(|_| "Clipboard copy failed.")?.success() { return Err("Clipboard copy failed.".into()); }
    Ok(())
}

#[tauri::command]
pub async fn cloud_share(app: AppHandle, id: String, content: String, format: String, clipboard: bool) -> Result<Value, String> {
    let s=get(&app,&id).await?;
    let _write=s.write_lock.lock().await;
    let m=s.meeting.lock().await.clone();
    let text=library::export_text(&m,&content,&format)?;
    let folder=s.folder.join("exports");
    tokio::fs::create_dir_all(&folder).await.map_err(|_| "Could not create the exports folder.")?;
    let path=folder.join(format!("{}-{}.{}",content,m.edit_version,if format=="markdown"{"md"}else{"txt"}));
    library::atomic_text(&path,&text)?;
    if clipboard { copy(&text).await?; }
    let result=json!({"meeting_id":id,"text":text,"path":path,"copied":clipboard,"content":content,"format":format});
    let _=app.emit("cloud-shared",json!({"id":id,"copied":clipboard,"content":content}));
    Ok(result)
}
