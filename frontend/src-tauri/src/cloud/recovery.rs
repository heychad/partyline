//! Retryable file transcription. Originals and the previous transcript are kept.
use super::{
    get, keys,
    library::{self, model::Recovery},
    Shared,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tauri::AppHandle;
use tokio_util::{io::ReaderStream, sync::CancellationToken};

#[derive(Default, Serialize, Deserialize)]
struct Job {
    ids: BTreeMap<String, String>,
    channel: String,
    expected_version: u64,
    generation: String,
}
async fn phase(s: &Shared, phase: &str, progress: u32) {
    let mut m = s.meeting.lock().await;
    m.recovery.phase = phase.into();
    m.recovery.progress = progress;
    drop(m);
    s.publish(true).await;
}
async fn request(response: reqwest::Response) -> Result<Value, String> {
    if !response.status().is_success() {
        return Err(format!(
            "AssemblyAI returned HTTP {}. Check your connection and credits, then retry recovery.",
            response.status().as_u16()
        ));
    }
    response
        .json()
        .await
        .map_err(|_| "Invalid recovery response. Your original files are unchanged.".into())
}
async fn run(
    s: Arc<Shared>,
    cancel: CancellationToken,
    channel: String,
    expected: u64,
) -> Result<(), String> {
    phase(&s, "Unlocking AssemblyAI", 0).await;
    let key = tokio::task::spawn_blocking(|| keys::read("assemblyai"))
        .await
        .map_err(|_| "Could not unlock AssemblyAI.")??;
    let path = s.folder.join("recovery-job.json");
    let mut job: Job = tokio::fs::read(&path)
        .await
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    if job.expected_version != expected || job.channel != channel || job.generation.is_empty() {
        job = Job {
            channel: channel.clone(),
            expected_version: expected,
            generation: uuid::Uuid::new_v4().simple().to_string(),
            ..Default::default()
        };
    }
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|_| "Could not start recovery.")?;
    let mut results = Vec::new();
    for (index, ch) in ["me", "remote"]
        .iter()
        .enumerate()
        .filter(|(_, c)| channel == "both" || **c == channel)
    {
        let name = if *ch == "me" {
            "microphone.wav"
        } else {
            "mac-audio.wav"
        };
        if !job.ids.contains_key(*ch) {
            phase(
                &s,
                &format!(
                    "Uploading {}",
                    if *ch == "me" {
                        "microphone"
                    } else {
                        "Mac audio"
                    }
                ),
                10 + index as u32 * 35,
            )
            .await;
            let audio = s.folder.join(name);
            let repair = audio.clone();
            tokio::task::spawn_blocking(move || super::archive::recover_wav(&repair))
                .await
                .map_err(|_| "Audio recovery failed.")??;
            let file = tokio::fs::File::open(&audio)
                .await
                .map_err(|_| "Saved audio is missing.")?;
            let bytes = file
                .metadata()
                .await
                .map_err(|_| "Could not read saved audio.")?
                .len();
            let response = client
                .post("https://api.assemblyai.com/v2/upload")
                .header("authorization", &key)
                .header("content-type", "application/octet-stream")
                .header("content-length", bytes)
                .body(reqwest::Body::wrap_stream(ReaderStream::new(file)))
                .send()
                .await
                .map_err(|_| "Audio upload failed. Retry recovery when the connection returns.")?;
            let upload = request(response).await?;
            let url = upload["upload_url"]
                .as_str()
                .ok_or("No upload URL returned.")?;
            let response=client.post("https://api.assemblyai.com/v2/transcript").header("authorization",&key).json(&json!({"audio_url":url,"speech_models":["universal-3-pro","universal-2"],"speaker_labels":*ch=="remote","language_detection":true})).send().await.map_err(|_|"Transcription submission was not confirmed. Check the provider before retrying to avoid a duplicate job.")?;
            let result = request(response).await?;
            let id = result["id"]
                .as_str()
                .ok_or("No transcript ID returned.")?
                .to_string();
            if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err("Invalid transcription ID.".into());
            }
            job.ids.insert(ch.to_string(), id);
            library::atomic_json(&path, &job)?;
        }
        phase(
            &s,
            &format!(
                "Transcribing {}",
                if *ch == "me" {
                    "microphone"
                } else {
                    "Mac audio"
                }
            ),
            25 + index as u32 * 35,
        )
        .await;
        let id = &job.ids[*ch];
        let result = loop {
            let response = client
                .get(format!("https://api.assemblyai.com/v2/transcript/{id}"))
                .header("authorization", &key)
                .send()
                .await
                .map_err(|_| {
                    "Recovery is waiting for a connection. Retry to resume the same provider job."
                })?;
            let result = request(response).await?;
            match result["status"].as_str() {
                Some("completed") => break result,
                Some("error") => {
                    job.ids.remove(*ch);
                    library::atomic_json(&path, &job)?;
                    return Err("AssemblyAI could not transcribe the file. Retry recovery; originals are unchanged.".into());
                }
                _ => {
                    tokio::select! {_=cancel.cancelled()=>return Err("Recovery cancelled. Provider work already submitted may still finish.".into()),_=tokio::time::sleep(Duration::from_secs(5))=>{}}
                }
            }
        };
        let result_path = s
            .folder
            .join(format!("recovered-{}-{}.json", job.generation, ch));
        library::atomic_json(&result_path, &result)?;
        results.push((ch.to_string(), result));
    }
    phase(&s, "Saving recovered transcript", 95).await;
    // One revision transaction: cancellation or failure cannot partly overwrite
    // the user's transcript. Edits made while the job runs reject the apply.
    let _guard = s.write_lock.lock().await;
    let mut m = s.meeting.lock().await;
    if m.edit_version != expected || m.deleted_at.is_some() {
        return Err("The meeting changed during recovery. Results are saved separately; retry after reviewing your edits.".into());
    }
    let mut next = m.clone();
    for (channel, result) in results {
        library::replace_recovered_channel(&mut next, &channel, &job.generation, &result)?;
    }
    let revisions = s.folder.join("revisions");
    std::fs::create_dir_all(&revisions)
        .map_err(|_| "Could not preserve the existing transcript.")?;
    library::atomic_json(&revisions.join(format!("{}.json", m.edit_version)), &*m)?;
    next.undo_version = Some(m.edit_version);
    next.edit_version += 1;
    next.revision += 1;
    next.speaker_revision += 1;
    next.notes_need_review = true;
    next.recovery_needed = channel != "both" && m.recovery_needed;
    next.recovery.status = "complete".into();
    next.recovery.phase = "Transcript recovered; refresh notes".into();
    next.recovery.progress = 100;
    next.recovery.error = None;
    library::atomic_json(&s.folder.join("meeting.json"), &next)?;
    *m = next;
    drop(m);
    drop(_guard);
    s.publish(false).await;
    Ok(())
}
#[tauri::command]
pub async fn cloud_recovery_start(
    app: AppHandle,
    id: String,
    channel: String,
    expected_version: u64,
) -> Result<Value, String> {
    if !["both", "me", "remote"].contains(&channel.as_str()) {
        return Err("Choose both, microphone, or Mac audio.".into());
    }
    let s = get(&app, &id).await?;
    let mut lock = s.recovery_cancel.lock().await;
    if lock.is_some() {
        return Err("Recovery is already running.".into());
    }
    {
        let mut m = s.meeting.lock().await;
        library::idle(&m)?;
        if m.deleted_at.is_some() {
            return Err("Restore the meeting first.".into());
        }
        if m.edit_version != expected_version {
            return Err("The meeting changed. Reload it before recovery.".into());
        }
        m.recovery = Recovery {
            status: "running".into(),
            phase: "Starting recovery".into(),
            channel: channel.clone(),
            ..Default::default()
        };
    }
    let cancel = CancellationToken::new();
    *lock = Some(cancel.clone());
    drop(lock);
    s.publish(true).await;
    tauri::async_runtime::spawn(async move {
        let result = tokio::select! {_=cancel.cancelled()=>Err("Recovery cancelled. Submitted provider work may still finish; retry resumes it.".to_string()),r=run(s.clone(),cancel.clone(),channel,expected_version)=>r};
        if let Err(e) = result {
            let mut m = s.meeting.lock().await;
            if m.recovery.status != "complete" {
                m.recovery.status = if cancel.is_cancelled() {
                    "cancelled"
                } else {
                    "failed"
                }
                .into();
                m.recovery.error = Some(e);
            }
        }
        *s.recovery_cancel.lock().await = None;
        s.publish(true).await;
    });
    Ok(json!({"accepted":true,"meeting_id":id}))
}
#[tauri::command]
pub async fn cloud_recovery_cancel(app: AppHandle, id: String) -> Result<Value, String> {
    let s = get(&app, &id).await?;
    let lock = s.recovery_cancel.lock().await;
    let cancel = lock.as_ref().ok_or("No recovery is running.")?;
    cancel.cancel();
    Ok(json!({"accepted":true}))
}
