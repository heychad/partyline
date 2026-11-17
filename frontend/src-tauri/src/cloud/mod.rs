//! Cloud recording: independent microphone and Mac audio, with local recovery files.
pub mod calendar;
mod assembly;
mod archive;
pub mod recovery;
pub mod preferences;
pub mod setup;
mod capture;
pub mod keys;
pub mod library;
pub mod operations;
pub mod chat;
pub mod backup;
pub mod clients;
mod notes;
use library::model::Meeting;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex as SyncMutex, OnceLock,
    },
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, Mutex, Notify};
use tokio_util::sync::CancellationToken;

static ACTIVE: SyncMutex<Option<Arc<Shared>>> = SyncMutex::new(None);
static LOADED: OnceLock<Mutex<HashMap<String, Arc<Shared>>>> = OnceLock::new();
fn loaded() -> &'static Mutex<HashMap<String, Arc<Shared>>> {
    LOADED.get_or_init(|| Mutex::new(HashMap::new()))
}
pub struct Shared {
    app: AppHandle,
    id: String,
    folder: PathBuf,
    meeting: Mutex<Meeting>,
    write_lock: Mutex<()>,
    stop: CancellationToken,
    audio_done: CancellationToken,
    done: CancellationToken,
    paused: Arc<AtomicBool>,
    notes_wake: Notify,
    notes_lock: Mutex<()>,
    force_notes: AtomicBool,
    recovery_cancel: Mutex<Option<CancellationToken>>,
}
impl Shared {
    fn new(app: AppHandle, folder: PathBuf, meeting: Meeting) -> Arc<Self> {
        Arc::new(Self {
            app,
            id: meeting.id.clone(),
            folder,
            meeting: Mutex::new(meeting),
            write_lock: Mutex::new(()),
            stop: CancellationToken::new(),
            audio_done: CancellationToken::new(),
            done: CancellationToken::new(),
            paused: Arc::new(AtomicBool::new(false)),
            notes_wake: Notify::new(),
            notes_lock: Mutex::new(()),
            force_notes: AtomicBool::new(false),
            recovery_cancel: Mutex::new(None),
        })
    }
    async fn publish(&self, persist: bool) {
        // Serialize publication too: a slow disk write must not publish an older snapshot last.
        let _guard = self.write_lock.lock().await;
        let mut m = self.meeting.lock().await.clone();
        if persist {
            m.notes_draft.clear();
            let result = async {
                let bytes = serde_json::to_vec_pretty(&m).map_err(|_| ())?;
                tokio::fs::write(self.folder.join("meeting.json.tmp"), bytes)
                    .await
                    .map_err(|_| ())?;
                tokio::fs::rename(
                    self.folder.join("meeting.json.tmp"),
                    self.folder.join("meeting.json"),
                )
                .await
                .map_err(|_| ())
            }
            .await;
            if result.is_err() {
                let error = "Could not save this meeting. Check available disk space.".to_string();
                let mut current = self.meeting.lock().await;
                if !current.errors.contains(&error) {
                    current.errors.push(error);
                }
            }
        }
        let current = self.meeting.lock().await.clone();
        let _ = self.app.emit("cloud-meeting", current);
    }
    async fn warning(&self, error: String) {
        {
            let mut m = self.meeting.lock().await;
            if !m.errors.contains(&error) {
                m.errors.push(error);
            }
        }
        self.publish(true).await;
    }
    async fn connection(&self, channel: &str, status: &str) {
        self.meeting
            .lock()
            .await
            .connection
            .insert(channel.into(), status.into());
        self.publish(false).await;
    }
}
fn root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|p| p.join("cloud-meetings"))
        .map_err(|_| "Could not locate PartyLine's data folder.".into())
}
fn safe_id(id: &str) -> Result<(), String> {
    uuid::Uuid::parse_str(id)
        .map(|_| ())
        .map_err(|_| "Invalid meeting ID.".into())
}
async fn get(app: &AppHandle, id: &str) -> Result<Arc<Shared>, String> {
    safe_id(id)?;
    let mut cache = loaded().lock().await;
    if let Some(s) = cache.get(id) {
        return Ok(s.clone());
    }
    let folder = root(app)?.join(id);
    let data = tokio::fs::read(folder.join("meeting.json"))
        .await
        .map_err(|_| "Could not read this meeting.".to_string())?;
    let mut meeting: Meeting = serde_json::from_slice(&data).map_err(|_| {
        "The saved meeting could not be read. Your audio files are still in its folder.".to_string()
    })?;
    if ["connecting", "recording", "paused", "finishing"].contains(&meeting.status.as_str()) {
        meeting.status = "interrupted".into();
        meeting.notes_status = "idle".into();
        meeting.notes_draft.clear();
        meeting.capture.recording = false;
        meeting.recovery_needed = true;
        let mut durations = Vec::new();
        for name in ["microphone.wav", "mac-audio.wav"] {
            if let Ok(duration) = archive::recover_wav(&folder.join(name)) { durations.push(duration); }
        }
        if !durations.is_empty() { meeting.duration = durations.into_iter().fold(meeting.duration, f64::max); }
        meeting.errors.push(
            "The app closed before this meeting finished. Captured audio is in its folder.".into(),
        );
    }
    if meeting.recovery.status == "running" {
        meeting.recovery.status = "interrupted".into();
        meeting.recovery.error = Some("Recovery was interrupted. Retry to resume the saved provider job.".into());
    }
    let s = Shared::new(app.clone(), folder, meeting);
    s.done.cancel();
    cache.insert(id.into(), s.clone());
    Ok(s)
}
pub fn active() -> bool {
    ACTIVE.lock().unwrap().is_some()
}
pub fn paused() -> bool {
    ACTIVE
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|s| s.paused.load(Ordering::SeqCst))
}
pub fn stop_active() {
    if let Some(s) = ACTIVE.lock().unwrap().as_ref() {
        s.stop.cancel();
    }
}
pub async fn finish_before_exit() {
    let s = ACTIVE.lock().unwrap().clone();
    if let Some(s) = s {
        s.stop.cancel();
        s.done.cancelled().await;
    }
}
#[tauri::command]
pub async fn cloud_current() -> Option<Meeting> {
    let s = ACTIVE.lock().unwrap().clone();
    if let Some(s) = s {
        Some(s.meeting.lock().await.clone())
    } else {
        None
    }
}
#[tauri::command]
pub async fn cloud_get(app: AppHandle, id: String) -> Result<Meeting, String> {
    Ok(get(&app, &id).await?.meeting.lock().await.clone())
}
#[tauri::command]
pub async fn cloud_list(app: AppHandle) -> Result<Vec<serde_json::Value>, String> {
    let path = root(&app)?;
    tokio::fs::create_dir_all(&path)
        .await
        .map_err(|_| "Could not open the meetings folder.".to_string())?;
    let mut entries = tokio::fs::read_dir(path)
        .await
        .map_err(|_| "Could not list meetings.".to_string())?;
    let mut list = Vec::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|_| "Could not list meetings.".to_string())?
    {
        let id = entry.file_name().to_string_lossy().into_owned();
        if let Ok(s) = get(&app, &id).await {
            let m = s.meeting.lock().await;
            if m.deleted_at.is_some() { continue; }
            list.push(serde_json::json!({"id":m.id,"title":m.title,"created_at":m.created_at,"status":m.status}));
        }
    }
    list.sort_by(|a, b| b["created_at"].as_str().cmp(&a["created_at"].as_str()));
    Ok(list)
}
#[tauri::command]
pub async fn cloud_start(
    app: AppHandle,
    title: String,
    participants: Vec<String>,
    microphone: Option<String>,
) -> Result<Meeting, String> {
    if crate::audio::recording_commands::is_recording().await {
        return Err("Stop the existing recording first.".into());
    }
    let title = title.trim();
    if title.len() > 200 {
        return Err("Use a shorter meeting title.".into());
    }
    let participants = participants
        .into_iter()
        .map(|s| s.trim().chars().take(80).collect::<String>())
        .filter(|s| !s.is_empty())
        .take(100)
        .collect();
    let m = Meeting::new(
        if title.is_empty() {
            format!(
                "Meeting · {}",
                chrono::Local::now().format("%b %-d, %-I:%M %p")
            )
        } else {
            title.into()
        },
        participants,
    );
    let folder = root(&app)?.join(&m.id);
    let s = Shared::new(app.clone(), folder, m.clone());
    {
        let mut current = ACTIVE.lock().unwrap();
        if current.is_some() || setup::RUNNING.load(Ordering::SeqCst) {
            return Err("A meeting or audio setup check is already running.".into());
        }
        *current = Some(s.clone());
    }
    if tokio::fs::create_dir_all(&s.folder).await.is_err() {
        *ACTIVE.lock().unwrap() = None;
        return Err("Could not create the recording folder.".into());
    }
    loaded().lock().await.insert(m.id.clone(), s.clone());
    s.publish(true).await;
    tauri::async_runtime::spawn(async move {
        run(s, microphone).await;
    });
    Ok(m)
}
async fn run(s: Arc<Shared>, microphone: Option<String>) {
    let result=async {
        let (mt,mr)=mpsc::channel(120);let (rt,rr)=mpsc::channel(120);
        let stream=s.clone();
        let jobs=tokio::spawn(async move {
            let key=tokio::select!{_ = stream.stop.cancelled()=>return,r=tokio::task::spawn_blocking(||keys::read("assemblyai"))=>r.unwrap_or_else(|_|Err("Could not unlock AssemblyAI.".into()))};
            match key {Ok(a)=>{tokio::join!(assembly::run(stream.clone(),"me",a.clone(),mr),assembly::run(stream.clone(),"remote",a,rr));},Err(e)=>{stream.meeting.lock().await.recovery_needed=true;stream.warning(format!("{e} Local audio continues; recover the transcript afterward.")).await;}}
        });
        let ns=s.clone();let njob=tokio::spawn(async move {
            let key=tokio::select!{_ = ns.audio_done.cancelled()=>return,r=tokio::task::spawn_blocking(||keys::read("openai"))=>r.unwrap_or_else(|_|Err("Could not unlock OpenAI.".into()))};
            match key {Ok(o)=>notes::run(ns,o).await,Err(e)=>{ns.meeting.lock().await.notes_error=Some(e);ns.publish(true).await;}}
        });
        let result=capture::run(s.clone(),microphone,[mt,rt]).await;
        if let Err(e)=&result{s.warning(e.clone()).await;}
        // Both streams drain final words and speaker revisions before final notes.
        let _=jobs.await;
        s.audio_done.cancel();s.notes_wake.notify_one();let _=njob.await;
        result
    }.await;
    if let Err(e) = result {
        s.warning(e).await;
        {let mut m=s.meeting.lock().await;m.status="interrupted".into();m.capture.recording=false;}
        use tauri_plugin_notification::NotificationExt;
        let _=s.app.notification().builder().title("PartyLine recording stopped").body("Audio capture stopped unexpectedly. Saved audio is preserved. Open PartyLine to review the error.").show();
        if let Some(window)=s.app.get_webview_window("main"){let _=window.request_user_attention(Some(tauri::UserAttentionType::Critical));}
    } else {
        s.meeting.lock().await.status = "complete".into();
    }
    s.publish(true).await;
    {
        let mut current = ACTIVE.lock().unwrap();
        if current.as_ref().is_some_and(|v| v.id == s.id) {
            *current = None;
        }
    }
    s.done.cancel();
    crate::tray::update_tray_menu(&s.app);
    backup::automatic(s.app.clone(),s.id.clone()).await;
}
#[tauri::command]
pub async fn cloud_stop() -> Result<(), String> {
    let s = ACTIVE
        .lock()
        .unwrap()
        .clone()
        .ok_or("No meeting is recording.")?;
    s.stop.cancel();
    s.meeting.lock().await.status = "finishing".into();
    s.publish(true).await;
    Ok(())
}
#[tauri::command]
pub async fn cloud_pause(paused: bool) -> Result<(), String> {
    let s = ACTIVE
        .lock()
        .unwrap()
        .clone()
        .ok_or("No meeting is recording.")?;
    {
        let mut m = s.meeting.lock().await;
        if !["recording", "paused"].contains(&m.status.as_str()) {
            return Err("Wait for the recording to start.".into());
        }
        m.status = if paused { "paused" } else { "recording" }.into();
    }
    s.paused.store(paused, Ordering::SeqCst);
    s.publish(true).await;
    crate::tray::update_tray_menu(&s.app);
    Ok(())
}
#[tauri::command]
pub async fn cloud_rename_speaker(
    app: AppHandle,
    id: String,
    speaker: String,
    name: String,
) -> Result<(), String> {
    let s = get(&app, &id).await?;
    if s.meeting.lock().await.deleted_at.is_some() { return Err("Restore this meeting before renaming speakers.".into()); }
    if speaker == "me" || speaker.ends_with(":PENDING") || speaker.ends_with(":UNKNOWN") {
        return Err("This speaker cannot be renamed yet.".into());
    }
    let name = name.trim();
    if name.chars().count() > 80 {
        return Err("Use a shorter name.".into());
    }
    {
        let mut m = s.meeting.lock().await;
        if !m
            .turns
            .values()
            .any(|t| t.speaker == speaker || t.words.iter().any(|w| w.speaker == speaker))
        {
            return Err("Speaker not found.".into());
        }
        if name.is_empty() {
            m.names.remove(&speaker);
        } else {
            m.names.insert(speaker, name.into());
        }
        m.revision += 1;
        m.speaker_revision += 1;
    }
    if s.audio_done.is_cancelled() && !s.done.is_cancelled() { s.force_notes.store(true, Ordering::SeqCst); }
    // Name corrections mark notes stale, but respect the user's refresh interval.
    s.publish(true).await;
    if !s.done.is_cancelled() {
        s.notes_wake.notify_one();
    } // Saved meetings offer an explicit notes refresh.
    Ok(())
}
#[tauri::command]
pub async fn cloud_refresh_notes(app: AppHandle, id: String) -> Result<(), String> {
    let s = get(&app, &id).await?;
    if s.meeting.lock().await.deleted_at.is_some() { return Err("Restore this meeting before refreshing its notes.".into()); }
    if !s.done.is_cancelled() {
        s.force_notes.store(true, Ordering::SeqCst);
        s.notes_wake.notify_one();
        return Ok(());
    }
    let key = keys::read("openai")?;
    let _notes = s.notes_lock.try_lock().map_err(|_| "Notes are already updating.")?;
    { let mut m=s.meeting.lock().await;
      if ["updating","finalizing"].contains(&m.notes_status.as_str()) { return Err("Notes are already updating.".into()); }
      m.notes_status="updating".into();m.notes_error=None;
    }
    s.publish(false).await;
    drop(_notes);
    tauri::async_runtime::spawn(async move {
        let _ = notes::generate(&s, &key, true, "").await;
    });
    Ok(())
}
#[tauri::command]
pub async fn cloud_reveal(app: AppHandle, id: String) -> Result<(), String> {
    safe_id(&id)?;
    let path = root(&app)?.join(id);
    if !path.is_dir() {
        return Err("Meeting folder not found.".into());
    }
    std::process::Command::new("open")
        .arg(path)
        .spawn()
        .map_err(|_| "Could not open the meeting folder.".to_string())?;
    Ok(())
}
#[tauri::command]
pub async fn cloud_export(app: AppHandle, id: String) -> Result<String, String> {
    let s = get(&app, &id).await?;
    let m = s.meeting.lock().await.clone();
    let text = library::export_text(&m, "both", "markdown")?;
    tokio::fs::write(s.folder.join("meeting.md"), &text)
        .await
        .map_err(|_| "Could not export this meeting.".to_string())?;
    Ok(text)
}

#[tauri::command]
pub async fn cloud_merge_speakers(app:AppHandle,id:String,sources:Vec<String>,target:String,expected_version:u64)->Result<Meeting,String>{
    let s=get(&app,&id).await?;{let mut m=s.meeting.lock().await;m.merge_speakers(&sources,&target,expected_version)?;}
    if s.audio_done.is_cancelled() && !s.done.is_cancelled() { s.force_notes.store(true, Ordering::SeqCst); }
    s.notes_wake.notify_one();s.publish(true).await;let result=s.meeting.lock().await.clone();Ok(result)
}
#[tauri::command]
pub async fn cloud_undo_speaker_merge(app:AppHandle,id:String,expected_version:u64)->Result<Meeting,String>{
    let s=get(&app,&id).await?;{let mut m=s.meeting.lock().await;m.undo_merge(expected_version)?;}
    if s.audio_done.is_cancelled() && !s.done.is_cancelled() { s.force_notes.store(true, Ordering::SeqCst); }
    s.notes_wake.notify_one();s.publish(true).await;let result=s.meeting.lock().await.clone();Ok(result)
}
