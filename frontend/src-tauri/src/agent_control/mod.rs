//! Opt-in agent control. Register `agent_control::init()` on the Tauri builder.
//! Access remains off until a human enables it in the main window.
pub mod core;
mod native;

use core::{Access, Service};
use native::Native;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tauri::{
    plugin::{Builder, TauriPlugin},
    Emitter, Listener, Manager, State, WebviewWindow,
};

struct Control {
    service: Arc<Service>,
    native: Arc<Native>,
    error: Arc<Mutex<Option<String>>>,
    shutdown: tokio::sync::watch::Sender<bool>,
}

fn local_window(window: &WebviewWindow) -> Result<(), String> {
    let url = window.url().map_err(|_| "Window unavailable.")?;
    let local = url.scheme() == "tauri"
        || (url.host_str() == Some("tauri.localhost"))
        || (cfg!(debug_assertions)
            && url.host_str() == Some("localhost")
            && url.port() == Some(3118));
    if window.label() == "main" && local {
        Ok(())
    } else {
        Err("Agent permissions are controlled from PartyLine's main window.".into())
    }
}

#[tauri::command]
fn permissions(window: WebviewWindow, state: State<'_, Control>) -> Result<Value, String> {
    local_window(&window)?;
    Ok(
        json!({"access":state.service.access(),"activity":state.service.activity(),"error":*state.error.lock().unwrap()}),
    )
}

#[tauri::command]
fn set_permissions(
    window: WebviewWindow,
    state: State<'_, Control>,
    access: Access,
) -> Result<(), String> {
    local_window(&window)?;
    state.service.set_access(access);
    let _ = state.native.app.emit("partyline-agent-permissions", access);
    Ok(())
}

#[tauri::command]
fn ui_result(
    window: WebviewWindow,
    state: State<'_, Control>,
    id: String,
    ok: bool,
) -> Result<(), String> {
    local_window(&window)?;
    state.native.ui_result(&id, ok);
    Ok(())
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct View {
    selected_id: Option<String>,
    follow: bool,
    connections_open: bool,
    ready: bool,
    draft: Draft,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Draft {
    title: String,
    participants: Vec<String>,
    microphone: String,
}

#[tauri::command]
fn ui_state(window: WebviewWindow, state: State<'_, Control>, view: View) -> Result<(), String> {
    local_window(&window)?;
    *state.native.view.lock().unwrap() = json!(view);
    Ok(())
}

pub fn init() -> TauriPlugin<tauri::Wry> {
    Builder::new("agent-control")
        .invoke_handler(tauri::generate_handler![permissions, set_permissions, ui_result, ui_state])
        .setup(|app, _| {
            let native = Native::new(app.clone());
            let service = Arc::new(Service::new(native.clone()));
            let error = Arc::new(Mutex::new(None));
            let (shutdown, receiver) = tokio::sync::watch::channel(false);
            let observer = native.clone();
            app.listen("cloud-levels", move |event| {
                if let Ok(mut value) = serde_json::from_str::<Value>(event.payload()) {
                    if !value.is_object() { return; }
                    value["observed_at_ms"] = json!(std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64);
                    *observer.levels.lock().unwrap() = Some((value, std::time::Instant::now()));
                }
            });
            app.manage(Control { service:service.clone(), native, error:error.clone(), shutdown });
            let directory = app.path().app_data_dir().ok().and_then(|path| {
                std::fs::create_dir_all(&path).ok()?;
                Some(path.join("agent-control"))
            });
            let notifier = app.clone();
            tauri::async_runtime::spawn(async move {
                #[cfg(unix)]
                match directory.and_then(|p| core::socket::Endpoint::bind(&p).ok()) {
                    Some(endpoint) => endpoint.serve(service, receiver).await,
                    None => {
                        let message = "Agent control could not open its private socket. Recording remains available.";
                        *error.lock().unwrap() = Some(message.into());
                        let _ = notifier.emit("partyline-agent-unavailable", message);
                    }
                }
                #[cfg(not(unix))]
                { let _ = (directory, receiver, service); *error.lock().unwrap() = Some("Agent control currently requires macOS.".into()); }
            });
            Ok(())
        })
        .on_event(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                if let Some(state) = app.try_state::<Control>() {
                    state.service.set_access(Access::Off);
                    let _ = state.shutdown.send(true);
                }
            }
        })
        .build()
}
