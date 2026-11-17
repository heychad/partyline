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
    pending: Mutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>,
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

    pub fn ui_result(&self, id: &str, ok: bool, result: Option<Value>, error: Option<String>) {
        if let Some(sender) = self.pending.lock().unwrap().remove(id) {
            let _ = sender.send(if ok { Ok(result.unwrap_or(json!({"ok":true}))) } else { Err(error.unwrap_or_else(|| "The window could not perform this action.".into())) });
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
        let result = tokio::time::timeout(Duration::from_secs(50), receiver).await;
        self.pending.lock().unwrap().remove(&id);
        match result {
            Ok(Ok(result)) => result,
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
            ("partyline_calendar", "status" | "import_client" | "configure_client" | "connect" | "cancel" | "refresh" | "select" | "disconnect" | "detection_configure" | "dismiss") => {
                crate::cloud::calendar::cloud_calendar_action(self.app.clone(),arguments).await
            }
            ("partyline_calendar", "link" | "unlink") => {
                let m=crate::cloud::calendar::cloud_calendar_link(self.app.clone(),id,arguments["expected_version"].as_u64().ok_or("Missing version.")?,if action=="link"{Some(text(&arguments,"event_key"))}else{None},arguments["use_details"].as_bool().unwrap_or(false)).await?;
                Ok(json!({"meeting":m}))
            }
            ("partyline_status", "get") => {
                let current = crate::cloud::cloud_current().await.map(|m| {
                    json!({
                        "id":m.id,"title":m.title,"status":m.status,"duration":m.duration,
                        "connection":m.connection,"errors":m.errors,"notes_status":m.notes_status,
                        "revision":m.revision,"notes_revision":m.notes_revision,
                        "capture":m.capture,"recovery_needed":m.recovery_needed,"recovery":m.recovery,
                    })
                });
                let keys = crate::cloud::keys::cloud_key_status();
                Ok(
                    json!({"app":"PartyLine","version":env!("CARGO_PKG_VERSION"),"current":current,
                    "view":self.view.lock().unwrap().clone(),
                    "connections":{"assemblyai":keys.assemblyai,"openai":keys.openai},
                    "capabilities":{"transcript_text_edit":true,"live_input_switch":false,
                        "meeting_delete":true,"recoverable_trash":true,"trim":true,"redaction":true,"playback":true,
                        "live_participant_list_edit":true,"google_calendar":true,"meeting_detection":true,"update_check":false},
                    "permission_scope":"Cloud meetings in this macOS app. Agent access is saved until changed in PartyLine."}),
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
                    "capture":current.as_ref().map(|m| &m.capture),
                    "recovery_needed":current.as_ref().map(|m| m.recovery_needed),
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
            ("partyline_recovery", "status") => {
                let m=crate::cloud::cloud_get(self.app.clone(),id).await?;
                Ok(json!({"meeting_id":m.id,"recovery":m.recovery,"recovery_needed":m.recovery_needed,"edit_version":m.edit_version,"capture":m.capture}))
            }
            ("partyline_recovery", "start") => crate::cloud::recovery::cloud_recovery_start(self.app.clone(),id,text(&arguments,"channel"),arguments["expected_version"].as_u64().unwrap()).await,
            ("partyline_recovery", "cancel") => crate::cloud::recovery::cloud_recovery_cancel(self.app.clone(),id).await,
            ("partyline_meetings", "library") => crate::cloud::operations::cloud_library(self.app.clone(),arguments["trash"].as_bool().unwrap_or(false),text(&arguments,"query")).await,
            ("partyline_meetings", "search") => crate::cloud::operations::cloud_search(self.app.clone(),text(&arguments,"query"),arguments["limit"].as_u64().unwrap_or(50) as usize).await,
            ("partyline_meetings", "update" | "edit_turn" | "trim" | "redact" | "unredact" | "trash" | "restore" | "undo") => {
                use crate::cloud::library::Edit;
                let edit=match action {
                    "update"=>Edit::Details{title:text(&arguments,"title"),participants:arguments["participants"].as_array().unwrap().iter().map(|p|p.as_str().unwrap().to_owned()).collect()},
                    "edit_turn"=>Edit::Transcript{turn_id:text(&arguments,"turn_id"),text:text(&arguments,"text"),start:arguments["start"].as_f64(),end:arguments["end"].as_f64()},
                    "trim"=>Edit::Trim{start:arguments["start"].as_f64().unwrap(),end:arguments["end"].as_f64().unwrap()},
                    "redact"=>Edit::Redact{start:arguments["start"].as_f64().unwrap(),end:arguments["end"].as_f64().unwrap()},
                    "unredact"=>Edit::Unredact{redaction_id:text(&arguments,"redaction_id")},
                    "trash"=>Edit::Trash,"restore"=>Edit::Restore,_=>Edit::Undo,
                };
                Ok(json!({"meeting":crate::cloud::operations::cloud_edit(self.app.clone(),id,arguments["expected_version"].as_u64().unwrap(),edit).await?}))
            }
            ("partyline_notes", "edit") => Ok(json!({"meeting":crate::cloud::operations::cloud_edit(self.app.clone(),id,arguments["expected_version"].as_u64().unwrap(),crate::cloud::library::Edit::Notes{text:text(&arguments,"text")}).await?})),
            ("partyline_export", "text") => crate::cloud::operations::cloud_share(self.app.clone(),id,text(&arguments,"content"),text(&arguments,"format"),arguments["clipboard"].as_bool().unwrap_or(false)).await,
            ("partyline_export", "audio") => crate::cloud::operations::cloud_audio(self.app.clone(),id,text(&arguments,"channel")).await,
            ("partyline_playback", "get") => Ok(json!({"playback":self.view.lock().unwrap().get("playback").cloned().unwrap_or(Value::Null)})),
            ("partyline_playback", "set") => { self.idle().await?; self.ui("playback",arguments).await },
            ("partyline_playback", "close") => self.ui("playback_close",json!({})).await,
            ("partyline_audio", "setup_status") => Ok(json!(crate::cloud::setup::cloud_setup_status())),
            ("partyline_audio", "setup_start") => { let mic=self.microphone(&arguments).await?; Ok(json!(crate::cloud::setup::cloud_setup_start(self.app.clone(),mic).await?)) },
            ("partyline_audio", "setup_cancel") => Ok(json!(crate::cloud::setup::cloud_setup_cancel())),
            ("partyline_workspace", "preferences") => Ok(json!(crate::cloud::preferences::cloud_preferences(self.app.clone()))),
            ("partyline_workspace", "configure") => Ok(json!(crate::cloud::preferences::cloud_preferences_update(self.app.clone(),arguments["patch"].clone()).await?)),
            ("partyline_workspace", "get") => Ok(json!({"workspace":self.view.lock().unwrap().get("workspace").cloned().unwrap_or(Value::Null)})),
            ("partyline_workspace", "set" | "outline" | "transcript") => self.ui("workspace",arguments).await,
            ("partyline_workspace", "outline_state") => Ok(json!(crate::cloud::preferences::cloud_outline(self.app.clone(),id).await?)),
            ("partyline_workspace", "outline_set") => {
                // The window checks the requested meeting and node against its rendered outline.
                let mut check = arguments.clone(); check["action"] = json!("outline_validate");
                self.ui("workspace",check).await?;
                let change = serde_json::from_value(arguments["change"].clone()).map_err(|_| "Invalid outline change.")?;
                Ok(json!(crate::cloud::preferences::cloud_outline_update(self.app.clone(),id,change).await?))
            },
            ("partyline_chat", "list") => crate::cloud::chat::cloud_chat_list(self.app.clone()).await,
            ("partyline_chat", "ask") => {
                let mut result=crate::cloud::chat::cloud_chat_ask(self.app.clone(),text(&arguments,"question"),arguments["meeting_id"].as_str().map(String::from)).await?;
                result["ui_synced"]=json!(self.ui("workspace",json!({"panel":"chat"})).await.is_ok());Ok(result)
            }
            ("partyline_chat", "cancel") => crate::cloud::chat::cloud_chat_cancel(id).await,
            ("partyline_backup", "settings") => Ok(json!(crate::cloud::backup::cloud_backup_settings(self.app.clone())?)),
            ("partyline_backup", "configure") => Ok(json!(crate::cloud::backup::cloud_backup_configure(self.app.clone(),arguments["automatic"].as_bool().unwrap(),Some(text(&arguments,"folder")))?)),
            ("partyline_backup", "choose_folder") => crate::cloud::backup::cloud_backup_choose_folder(self.app.clone()).await,
            ("partyline_backup", "status") => crate::cloud::backup::cloud_backup_status(self.app.clone()),
            ("partyline_backup", "start") => crate::cloud::backup::cloud_backup_start(self.app.clone(),id).await,
            ("partyline_clients", "status") => crate::cloud::clients::cloud_client_status(self.app.clone()),
            ("partyline_clients", "setup") => crate::cloud::clients::cloud_client_setup(self.app.clone(),text(&arguments,"client")).await,
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
                result["speaker_aliases"] = json!(m.speaker_aliases);
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
                    speakers.insert(m.canonical_speaker(&turn.speaker).to_owned());
                    speakers.extend(turn.words.iter().map(|word| m.canonical_speaker(&word.speaker).to_owned()));
                }
                let speakers: Vec<_> = speakers.into_iter().map(|speaker| json!({
                    "id":speaker,"display_name":m.display_name(&speaker),"confirmed_name":m.names.get(&speaker),
                    "can_rename":speaker != "me" && !speaker.ends_with(":PENDING") && !speaker.ends_with(":UNKNOWN"),
                })).collect();
                Ok(
                    json!({"speakers":speakers,"participant_suggestions":m.participants,"revision":m.revision,"speaker_aliases":m.speaker_aliases,"speaker_edit_version":m.speaker_edit_version,"can_undo_merge":m.previous_aliases.is_some()}),
                )
            }
            ("partyline_speakers", "merge") => Ok(json!(crate::cloud::cloud_merge_speakers(self.app.clone(),id,arguments["sources"].as_array().unwrap().iter().map(|v|v.as_str().unwrap().to_string()).collect(),text(&arguments,"target"),arguments["expected_version"].as_u64().unwrap()).await?)),
            ("partyline_speakers", "undo_merge") => Ok(json!(crate::cloud::cloud_undo_speaker_merge(self.app.clone(),id,arguments["expected_version"].as_u64().unwrap()).await?)),
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
