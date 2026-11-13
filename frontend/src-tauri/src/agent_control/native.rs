use super::core::{Activity, Backend};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::{
    collections::{BTreeSet, HashMap},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::{io::AsyncWriteExt, sync::oneshot};

pub struct Native {
    pub app: AppHandle,
    pub levels: Mutex<Option<(Value, Instant)>>,
    pub view: Mutex<Value>,
    pending: Mutex<HashMap<String, oneshot::Sender<bool>>>,
}

impl Native {
    pub fn new(app: AppHandle) -> Arc<Self> {
        Arc::new(Self {
            app,
            levels: Mutex::new(None),
            view: Mutex::new(Value::Null),
            pending: Mutex::new(HashMap::new()),
        })
    }

    pub fn ui_result(&self, id: &str, ok: bool) {
        if let Some(sender) = self.pending.lock().unwrap().remove(id) {
            let _ = sender.send(ok);
        }
    }

    async fn ui(&self, action: &str, arguments: Value) -> Result<Value, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().unwrap().insert(id.clone(), sender);
        if self
            .app
            .emit_to(
                "main",
                "partyline-agent-ui",
                json!({"id":id,"action":action,"arguments":arguments}),
            )
            .is_err()
        {
            self.pending.lock().unwrap().remove(&id);
            return Err("Could not reach the PartyLine window.".into());
        }
        let result = tokio::time::timeout(Duration::from_secs(5), receiver).await;
        self.pending.lock().unwrap().remove(&id);
        match result {
            Ok(Ok(true)) => Ok(json!({"ok":true})),
            Ok(Ok(false)) => Err("The window could not perform this action. Check PartyLine and retry.".into()),
            _ => Err("PartyLine's window did not acknowledge the action. Open the window and check its state before retrying.".into()),
        }
    }

    async fn idle(&self) -> Result<(), String> {
        if crate::cloud::cloud_current().await.is_some() {
            Err("Wait for the current meeting to finish.".into())
        } else {
            Ok(())
        }
    }

    async fn expected(&self, arguments: &Value) -> Result<String, String> {
        let current = crate::cloud::cloud_current()
            .await
            .ok_or("No meeting is recording.")?;
        if arguments["expected_meeting_id"].as_str() != Some(&current.id) {
            return Err("The active meeting changed. Read app status before retrying.".into());
        }
        Ok(current.id)
    }

    async fn microphone(&self, arguments: &Value) -> Result<Option<String>, String> {
        let name = arguments["microphone"].as_str().unwrap_or("");
        if name.is_empty() {
            return Ok(None);
        }
        let devices = crate::get_audio_devices().await?;
        if !devices
            .iter()
            .any(|d| d.name == name && d.device_type == crate::audio::devices::DeviceType::Input)
        {
            return Err(
                "Microphone not found. List inputs and choose an available microphone.".into(),
            );
        }
        Ok(Some(name.into()))
    }

    fn connection_changed(&self) -> Value {
        let status = crate::cloud::keys::cloud_key_status();
        let value = json!({"assemblyai":status.assemblyai,"openai":status.openai});
        let _ = self.app.emit("partyline-agent-connections", value.clone());
        value
    }
}

fn text(arguments: &Value, key: &str) -> String {
    arguments[key].as_str().unwrap_or("").into()
}
fn page(items: Vec<Value>, arguments: &Value) -> Value {
    let offset = arguments["offset"].as_u64().unwrap_or(0) as usize;
    let limit = arguments["limit"].as_u64().unwrap_or(100) as usize;
    let total = items.len();
    let end = offset.saturating_add(limit).min(total);
    json!({"items":items.into_iter().skip(offset).take(limit).collect::<Vec<_>>(),
        "total":total,"offset":offset,"next_offset":if end < total {Some(end)} else {None}})
}

#[async_trait]
impl Backend for Native {
    fn activity(&self, entry: &Activity) {
        let _ = self.app.emit("partyline-agent-activity", entry);
    }

    async fn call(&self, tool: &str, arguments: Value) -> Result<Value, String> {
        let action = arguments["action"].as_str().ok_or("Missing action.")?;
        let id = text(&arguments, "id");
        match (tool, action) {
            ("partyline_status", "get") => {
                let current = crate::cloud::cloud_current().await.map(|m| {
                    json!({
                        "id":m.id,"title":m.title,"status":m.status,"duration":m.duration,
                        "connection":m.connection,"errors":m.errors,"notes_status":m.notes_status,
                        "revision":m.revision,"notes_revision":m.notes_revision,
                    })
                });
                let keys = crate::cloud::keys::cloud_key_status();
                Ok(
                    json!({"app":"PartyLine","version":env!("CARGO_PKG_VERSION"),"current":current,
                    "view":self.view.lock().unwrap().clone(),
                    "connections":{"assemblyai":keys.assemblyai,"openai":keys.openai},
                    "capabilities":{"transcript_text_edit":false,"live_input_switch":false,
                        "meeting_delete":false,"live_participant_list_edit":false,"update_check":false},
                    "permission_scope":"Cloud meetings in this macOS app. Agent access resets to off on launch."}),
                )
            }
            ("partyline_status", "show") => {
                crate::tray::focus_main_window(&self.app);
                Ok(json!({"ok":true}))
            }
            ("partyline_status", "hide") => {
                self.app
                    .get_webview_window("main")
                    .ok_or("Window not found.")?
                    .hide()
                    .map_err(|_| "Could not hide PartyLine.")?;
                Ok(json!({"ok":true}))
            }
            ("partyline_status", "quit") => {
                if crate::cloud::cloud_current().await.is_some() {
                    self.expected(&arguments).await?;
                }
                let app = self.app.clone();
                tauri::async_runtime::spawn(async move {
                    // Let the socket return acceptance; the existing ExitRequested handler drains capture.
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    app.exit(0);
                });
                Ok(json!({"accepted":true}))
            }
            ("partyline_audio", "list") => {
                let inputs = crate::get_audio_devices()
                    .await?
                    .into_iter()
                    .filter(|d| d.device_type == crate::audio::devices::DeviceType::Input)
                    .collect::<Vec<_>>();
                Ok(json!({"inputs":inputs,"system_default":"","live_switch_supported":false}))
            }
            ("partyline_audio", "diagnostics") => {
                let current = crate::cloud::cloud_current().await;
                let levels = self.levels.lock().unwrap();
                let sample = levels
                    .as_ref()
                    .filter(|(v, _)| current.as_ref().is_some_and(|m| v["id"] == m.id));
                let age_ms = sample.map(|(_, at)| at.elapsed().as_millis() as u64);
                Ok(json!({"meeting_id":current.as_ref().map(|m| &m.id),
                    "status":current.as_ref().map(|m| &m.status),
                    "sample":sample.map(|(v,_)| v),"sample_age_ms":age_ms,
                    "fresh":age_ms.is_some_and(|age| age <= 2000),
                    "connection":current.as_ref().map(|m| &m.connection),
                    "warnings":current.as_ref().map(|m| &m.errors),
                    "active_microphone":null,
                    "active_microphone_known":false,
                    "note":"Levels measure received signal, not permission or transcript accuracy. The capture API does not report the active input name."}))
            }
            ("partyline_audio", "select") => {
                self.idle().await?;
                self.microphone(&arguments).await?;
                self.ui("prepare", json!({"microphone":arguments["microphone"]}))
                    .await
            }
            ("partyline_meetings", "list") => Ok(page(
                crate::cloud::cloud_list(self.app.clone()).await?,
                &arguments,
            )),
            ("partyline_meetings", "get") => {
                Ok(json!(crate::cloud::cloud_get(self.app.clone(), id).await?))
            }
            ("partyline_meetings", "transcript") => {
                let m = crate::cloud::cloud_get(self.app.clone(), id).await?;
                let final_only = arguments["final_only"].as_bool().unwrap_or(false);
                let items = m
                    .ordered_turns()
                    .into_iter()
                    .filter(|t| !final_only || t.final_text)
                    .map(|t| json!(t))
                    .collect();
                let mut result = page(items, &arguments);
                result["names"] = json!(m.names);
                result["revision"] = json!(m.revision);
                result["meeting_id"] = json!(m.id);
                Ok(result)
            }
            ("partyline_meetings", "open") => {
                if let Some(current) = crate::cloud::cloud_current().await {
                    if current.id != id {
                        return Err("Finish the active meeting before opening history.".into());
                    }
                }
                crate::cloud::cloud_get(self.app.clone(), id.clone()).await?;
                self.ui("open", json!({"id":id})).await
            }
            ("partyline_meetings", "new") => {
                self.idle().await?;
                self.ui("new", json!({})).await
            }
            ("partyline_meetings", "prepare") => {
                self.idle().await?;
                self.microphone(&arguments).await?;
                self.ui("prepare", arguments).await
            }
            ("partyline_meetings", "follow") => self.ui("follow", arguments).await,
            ("partyline_meetings", "dismiss") => self.ui("dismiss", arguments).await,
            ("partyline_recording", "start") => {
                let microphone = self.microphone(&arguments).await?;
                let participants = arguments["participants"]
                    .as_array()
                    .map(|p| {
                        p.iter()
                            .filter_map(Value::as_str)
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default();
                let m = crate::cloud::cloud_start(
                    self.app.clone(),
                    text(&arguments, "title"),
                    participants,
                    microphone,
                )
                .await?;
                // State changes already emit cloud-meeting. Also select the meeting if history was open.
                let ui_synced = self.ui("open", json!({"id":m.id})).await.is_ok();
                Ok(
                    json!({"accepted":true,"meeting_id":m.id,"ui_synced":ui_synced,
                    "meeting":crate::cloud::cloud_get(self.app.clone(),m.id).await?}),
                )
            }
            ("partyline_recording", "pause" | "resume" | "stop") => {
                let id = self.expected(&arguments).await?;
                if action == "stop" {
                    crate::cloud::cloud_stop().await?;
                } else {
                    crate::cloud::cloud_pause(action == "pause").await?;
                }
                Ok(json!({"accepted":true,"meeting_id":id}))
            }
            ("partyline_speakers", "list") => {
                let m = crate::cloud::cloud_get(self.app.clone(), id).await?;
                let mut speakers = BTreeSet::new();
                for turn in m.turns.values() {
                    speakers.insert(turn.speaker.clone());
                    speakers.extend(turn.words.iter().map(|word| word.speaker.clone()));
                }
                let speakers: Vec<_> = speakers.into_iter().map(|speaker| json!({
                    "id":speaker,"display_name":m.display_name(&speaker),"confirmed_name":m.names.get(&speaker),
                    "can_rename":speaker != "me" && !speaker.ends_with(":PENDING") && !speaker.ends_with(":UNKNOWN"),
                })).collect();
                Ok(
                    json!({"speakers":speakers,"participant_suggestions":m.participants,"revision":m.revision}),
                )
            }
            ("partyline_speakers", "rename") => {
                crate::cloud::cloud_rename_speaker(
                    self.app.clone(),
                    id.clone(),
                    text(&arguments, "speaker"),
                    text(&arguments, "name"),
                )
                .await?;
                Ok(json!({"ok":true,"meeting_id":id}))
            }
            ("partyline_notes", "get") => {
                let m = crate::cloud::cloud_get(self.app.clone(), id).await?;
                Ok(
                    json!({"meeting_id":m.id,"notes":m.notes,"draft":m.notes_draft,"status":m.notes_status,
                    "error":m.notes_error,"revision":m.revision,"notes_revision":m.notes_revision,
                    "stale":m.notes_revision < m.revision,"meeting_status":m.status}),
                )
            }
            ("partyline_notes", "refresh") => {
                crate::cloud::cloud_refresh_notes(self.app.clone(), id).await?;
                Ok(json!({"accepted":true}))
            }
            ("partyline_export", "reveal") => {
                self.idle().await?;
                crate::cloud::cloud_reveal(self.app.clone(), id).await?;
                Ok(json!({"ok":true}))
            }
            ("partyline_export", "markdown" | "copy") => {
                let markdown = crate::cloud::cloud_export(self.app.clone(), id.clone()).await?;
                if action == "copy" {
                    let mut child = tokio::process::Command::new("/usr/bin/pbcopy")
                        .stdin(std::process::Stdio::piped())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                        .map_err(|_| "Export saved, but the clipboard could not be opened.")?;
                    let mut input = child.stdin.take().ok_or("Clipboard input unavailable.")?;
                    input
                        .write_all(markdown.as_bytes())
                        .await
                        .map_err(|_| "Export saved, but clipboard copy failed.")?;
                    drop(input);
                    if !child
                        .wait()
                        .await
                        .map_err(|_| "Clipboard copy failed.")?
                        .success()
                    {
                        return Err("Export saved, but clipboard copy failed.".into());
                    }
                    let _ = self.ui("copied", json!({})).await;
                }
                Ok(
                    json!({"markdown":markdown,"saved":true,"copied":action=="copy","meeting_id":id}),
                )
            }
            ("partyline_connections", "status") => {
                Ok(json!(crate::cloud::keys::cloud_key_status()))
            }
            ("partyline_connections", "save") => {
                crate::cloud::keys::cloud_save_keys(
                    arguments["assemblyai"].as_str().map(String::from),
                    arguments["openai"].as_str().map(String::from),
                )?;
                let status = self.connection_changed();
                if arguments["test"].as_bool().unwrap_or(false) {
                    crate::cloud::keys::cloud_test_keys().await?;
                }
                Ok(status)
            }
            ("partyline_connections", "test") => {
                crate::cloud::keys::cloud_test_keys().await?;
                Ok(self.connection_changed())
            }
            ("partyline_connections", "remove") => {
                crate::cloud::keys::cloud_remove_key(text(&arguments, "provider"))?;
                Ok(self.connection_changed())
            }
            ("partyline_connections", "open" | "close") => {
                self.ui("connections", json!({"open":action=="open"}))
                    .await?;
                Ok(json!(crate::cloud::keys::cloud_key_status()))
            }
            _ => Err("Unsupported PartyLine action.".into()),
        }
    }
}
