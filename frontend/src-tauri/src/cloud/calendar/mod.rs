//! Read-only Google calendars and optional local call suggestions.
mod detection;
mod oauth;
use super::library::{
    self,
    calendar_model::{Account, Calendar, Detector, Event, Settings, Signal},
    model::Meeting,
    Edit,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;
static STATE: OnceLock<Mutex<State>> = OnceLock::new();
static WORK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
#[derive(Default)]
struct State {
    settings: Settings,
    loaded: bool,
    events: Vec<Event>,
    tokens: BTreeMap<String, oauth::Tokens>,
    errors: BTreeMap<String, String>,
    phase: String,
    error: Option<String>,
    last_sync: Option<String>,
    cancel: Option<CancellationToken>,
    detector: Detector,
    signals: Vec<Signal>,
    detection_error: Option<String>,
    last_scan: Option<String>,
}
fn state() -> std::sync::MutexGuard<'static, State> {
    STATE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap()
}
fn path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|_| "App data folder unavailable.")?
        .join("google-calendar.json"))
}
fn load(app: &AppHandle) -> Result<(), String> {
    let mut s = state();
    if !s.loaded {
        let path = path(app)?;
        if path.exists() {
            s.settings = serde_json::from_slice(
                &std::fs::read(path).map_err(|_| "Could not read calendar settings.")?,
            )
            .map_err(|_| "Calendar settings are invalid; existing data was kept.")?;
        }
        s.loaded = true;
    }
    Ok(())
}
fn persist(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    library::atomic_json(&path(app)?, settings)
}
fn snapshot() -> Value {
    let s = state();
    json!({"configured":!s.settings.client_id.is_empty(),"accounts":s.settings.accounts,"events":s.events,"account_errors":s.errors,"phase":if s.phase.is_empty(){"idle"}else{&s.phase},"error":s.error,"last_sync":s.last_sync,"detection":{"enabled":s.settings.detection_enabled,"signals":s.signals,"error":s.detection_error,"last_scan":s.last_scan},"window":{"past_hours":12,"future_days":7}})
}
fn publish(app: &AppHandle) {
    let _ = app.emit("cloud-calendar", snapshot());
}
fn failure(app: &AppHandle, error: Option<String>) {
    let mut s = state();
    s.error = error;
    s.phase = "idle".into();
    s.cancel = None;
    drop(s);
    publish(app);
}
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str, String> {
    v[k].as_str().ok_or_else(|| format!("Missing {k}."))
}
fn account_id(v: &Value) -> Result<String, String> {
    let id = text(v, "account_id")?;
    if !state().settings.accounts.iter().any(|a| a.id == id) {
        return Err("Google account not found. Refresh connections.".into());
    }
    Ok(id.into())
}
fn bounded_strings(v: &Value, k: &str, max: usize) -> Result<Vec<String>, String> {
    let a = v[k]
        .as_array()
        .ok_or("Choose calendars from this account.")?;
    if a.len() > max {
        return Err("Choose at most 50 calendars.".into());
    }
    a.iter()
        .map(|v| {
            v.as_str()
                .filter(|s| s.len() <= 1024)
                .map(str::to_owned)
                .ok_or("Invalid calendar selection.".into())
        })
        .collect()
}
async fn get_json(url: url::Url, token: &str) -> Result<Value, String> {
    let r = oauth::client()?
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| "Google Calendar could not be reached. Check your connection and refresh.")?;
    let status = r.status();
    if !status.is_success() {
        return Err(match status.as_u16(){401=>"Google access expired or was revoked. Reconnect this account.",403=>"Google denied calendar access. Check Calendar API enablement, consent, and workspace policy.",429=>"Google's request limit was reached. Wait a minute and refresh.",_=>"Google Calendar returned an error. Try refreshing."}.into());
    }
    r.json()
        .await
        .map_err(|_| "Google returned an invalid calendar response.".into())
}
async fn pages(mut url: url::Url, token: &str) -> Result<Vec<Value>, String> {
    let base = url.clone();
    let mut items = vec![];
    for _ in 0..20 {
        let page = get_json(url, token).await?;
        if let Some(rows) = page["items"].as_array() {
            items.extend(rows.iter().cloned());
        }
        if let Some(next) = page["nextPageToken"].as_str() {
            url = base.clone();
            url.query_pairs_mut().append_pair("pageToken", next);
        } else {
            return Ok(items);
        }
    }
    Err("This calendar exceeds the current sync limit. Choose fewer calendars or a less busy calendar.".into())
}
async fn access(client_id: &str, account: &Account) -> Result<String, String> {
    let cached = { state().tokens.get(&account.id).cloned() };
    let token = match cached {
        Some(t) => t,
        None => {
            let id = account.id.clone();
            tokio::task::spawn_blocking(move || {
                oauth::secret(&id).and_then(|s| {
                    serde_json::from_str::<oauth::Tokens>(&s).map_err(|_| {
                        "Invalid saved Google connection. Reconnect this account.".into()
                    })
                })
            })
            .await
            .map_err(|_| "Could not unlock Google connection.")??
        }
    };
    let token = if token.expires_at <= chrono::Utc::now().timestamp() + 60 {
        let t = oauth::refresh(client_id, &token).await?;
        let id = account.id.clone();
        let encoded =
            serde_json::to_string(&t).map_err(|_| "Could not save refreshed Google connection.")?;
        tokio::task::spawn_blocking(move || oauth::save(&id, &encoded))
            .await
            .map_err(|_| "Could not save refreshed Google connection.")??;
        t
    } else {
        token
    };
    let result = token.access_token.clone();
    state().tokens.insert(account.id.clone(), token);
    Ok(result)
}
async fn sync_account(
    client_id: &str,
    account: &Account,
) -> Result<(Vec<Calendar>, Vec<Event>), String> {
    let token = access(client_id, account).await?;
    let calendars = pages(
        url::Url::parse(
            "https://www.googleapis.com/calendar/v3/users/me/calendarList?maxResults=250",
        )
        .unwrap(),
        &token,
    )
    .await?
    .into_iter()
    .filter_map(|v| {
        Some(Calendar {
            id: v["id"].as_str()?.into(),
            name: v["summary"]
                .as_str()
                .unwrap_or("Calendar")
                .chars()
                .take(200)
                .collect(),
            primary: v["primary"] == true,
        })
    })
    .collect::<Vec<_>>();
    let now = chrono::Utc::now();
    let from = (now - chrono::Duration::hours(12)).to_rfc3339();
    let until = (now + chrono::Duration::days(7)).to_rfc3339();
    let mut events = vec![];
    for id in &account.selected {
        // "primary" is Google's stable per-account alias, including when the calendar's real ID is an email.
        if id != "primary" && !calendars.iter().any(|c| &c.id == id) {
            continue;
        }
        let mut url = url::Url::parse("https://www.googleapis.com/calendar/v3/calendars/").unwrap();
        url.path_segments_mut()
            .unwrap()
            .pop_if_empty()
            .push(id)
            .push("events");
        url.query_pairs_mut().extend_pairs([
            ("timeMin", from.as_str()),
            ("timeMax", until.as_str()),
            ("singleEvents", "true"),
            ("orderBy", "startTime"),
            ("maxResults", "2500"),
            ("showDeleted", "false"),
        ]);
        for v in pages(url, &token).await? {
            if let Some(event) = library::calendar_model::event(&account.id, id, &v) {
                events.push(event);
                if events.len() > 4000 {
                    return Err("This account has too many events in the sync window. Select fewer calendars.".into());
                }
            }
        }
    }
    Ok((calendars, events))
}
fn apply_sync_result(s: &mut State, id: &str, result: Result<(Vec<Calendar>, Vec<Event>), String>) {
    s.events.retain(|e| e.account_id != id);
    match result {
        Ok((calendars, events)) => {
            s.errors.remove(id);
            s.events.extend(events);
            if let Some(a) = s.settings.accounts.iter_mut().find(|a| a.id == id) {
                a.calendars = calendars;
            }
        }
        Err(error) => {
            s.errors.insert(id.into(), error);
        }
    }
}
async fn sync(app: &AppHandle) {
    let settings = state().settings.clone();
    for account in settings.accounts {
        let result = sync_account(&settings.client_id, &account).await;
        let mut s = state();
        apply_sync_result(&mut s, &account.id, result);
    }
    let mut s = state();
    s.events.sort_by(|a, b| {
        chrono::DateTime::parse_from_rfc3339(&a.start)
            .ok()
            .cmp(&chrono::DateTime::parse_from_rfc3339(&b.start).ok())
            .then(a.key.cmp(&b.key))
    });
    s.last_sync = Some(chrono::Utc::now().to_rfc3339());
    if let Err(e) = persist(app, &s.settings) {
        s.error = Some(e);
    }
    s.phase = "idle".into();
    drop(s);
    publish(app);
}
fn start_sync(app: AppHandle) -> Result<(), String> {
    let lock = WORK
        .try_lock()
        .map_err(|_| "Calendar is busy. Wait for it to finish or cancel sign-in.")?;
    state().phase = "refreshing".into();
    state().error = None;
    publish(&app);
    tauri::async_runtime::spawn(async move {
        let _guard = lock;
        sync(&app).await;
    });
    Ok(())
}

async fn save_client(app: &AppHandle, client_id: &str, client_secret: &str) -> Result<(), String> {
    let _lock = WORK
        .try_lock()
        .map_err(|_| "Wait for calendar activity to finish.")?;
    let id = client_id.trim();
    let secret = client_secret.trim();
    if id.len() > 512
        || !id.ends_with(".apps.googleusercontent.com")
        || id.chars().any(char::is_whitespace)
        || secret.is_empty()
        || secret.len() > 512
        || secret.chars().any(char::is_whitespace)
    {
        return Err("Enter the ID and secret from a Google Desktop app OAuth client.".into());
    }
    let mut settings = state().settings.clone();
    if !settings.accounts.is_empty() {
        return Err("Remove connected accounts before replacing the OAuth client.".into());
    }
    let secret = secret.to_owned();
    tokio::task::spawn_blocking(move || oauth::save("client-secret", &secret))
        .await
        .map_err(|_| "Could not save OAuth client.")??;
    settings.client_id = id.into();
    persist(&app, &settings)?;
    state().settings = settings;
    state().error = None;

    Ok(())
}

#[tauri::command]
pub async fn cloud_calendar_action(app: AppHandle, request: Value) -> Result<Value, String> {
    load(&app)?;
    let action = text(&request, "action")?;
    match action {
        "status" => {}
        "configure_client" => {
            save_client(
                &app,
                text(&request, "client_id")?,
                text(&request, "client_secret")?,
            )
            .await?;
        }
        "import_client" => {
            use tauri_plugin_dialog::DialogExt;
            let (send, receive) = tokio::sync::oneshot::channel();
            app.dialog()
                .file()
                .set_title("Choose the Google Desktop OAuth client JSON")
                .add_filter("Google OAuth client", &["json"])
                .pick_file(move |file| {
                    let _ = send.send(file);
                });
            if let Some(file) = receive
                .await
                .map_err(|_| "OAuth file chooser closed unexpectedly.")?
            {
                let path = file
                    .into_path()
                    .map_err(|_| "Choose a local OAuth client JSON file.")?;
                if tokio::fs::metadata(&path)
                    .await
                    .map_err(|_| "Could not read the selected OAuth client file.")?
                    .len()
                    > 65536
                {
                    return Err(
                        "Choose the small Desktop OAuth client JSON from Google Cloud Console."
                            .into(),
                    );
                }
                let bytes = tokio::fs::read(path)
                    .await
                    .map_err(|_| "Could not read the selected OAuth client file.")?;
                let (id, secret) = oauth::desktop_config(&bytes)?;
                save_client(&app, &id, &secret).await?;
            }
        }
        "connect" => {
            let lock = WORK
                .try_lock()
                .map_err(|_| "Calendar is busy. Wait or cancel the existing sign-in.")?;
            let expected = if request.get("account_id").is_some() {
                Some(account_id(&request)?)
            } else {
                None
            };
            let hint = expected.as_ref().and_then(|id| {
                state()
                    .settings
                    .accounts
                    .iter()
                    .find(|a| &a.id == id)
                    .map(|a| a.email.clone())
            });
            let client_id = state().settings.client_id.clone();
            if client_id.is_empty() {
                return Err("Set up the Google OAuth client first.".into());
            }
            if expected.is_none() && state().settings.accounts.len() >= 20 {
                return Err("PartyLine supports up to 20 Google accounts.".into());
            }
            let cancel = CancellationToken::new();
            {
                let mut s = state();
                s.phase = "awaiting_google".into();
                s.error = None;
                s.cancel = Some(cancel.clone());
            }
            publish(&app);
            let target = app.clone();
            tauri::async_runtime::spawn(async move {
                let _guard = lock;
                let result=async{
     let (id,email,tokens)=oauth::authorize(&client_id,hint.as_deref(),cancel.clone()).await?;
     if expected.as_ref().is_some_and(|value|value!=&id){return Err("You selected a different Google account. Reconnect the original account, or use Add Google account.".into());}
     if cancel.is_cancelled(){return Err("Google sign-in cancelled.".into());}
     {let mut s=state();s.phase="saving_account".into();s.cancel=None;}publish(&target);
     let encoded=serde_json::to_string(&tokens).map_err(|_|"Could not save Google connection.")?;let key=id.clone();tokio::task::spawn_blocking(move||oauth::save(&key,&encoded)).await.map_err(|_|"Could not save Google connection.")??;
     let mut s=state();let mut settings=s.settings.clone();if let Some(a)=settings.accounts.iter_mut().find(|a|a.id==id){a.email=email;}else{settings.accounts.push(Account{id:id.clone(),email,selected:vec!["primary".into()],calendars:vec![]});}persist(&target,&settings)?;s.settings=settings;s.tokens.insert(id,tokens);s.phase="refreshing".into();drop(s);Ok::<_,String>(())
    }.await;
                match result {
                    Ok(()) => sync(&target).await,
                    Err(e) => failure(&target, Some(e)),
                }
            });
        }
        "cancel" => {
            let mut s = state();
            if let Some(c) = &s.cancel {
                c.cancel();
                s.phase = "cancelling".into();
            } else {
                return Err("No cancellable Google sign-in is pending.".into());
            }
        }
        "refresh" => {
            start_sync(app.clone())?;
        }
        "select" => {
            let _lock = WORK
                .try_lock()
                .map_err(|_| "Wait for calendar activity to finish.")?;
            let id = account_id(&request)?;
            let selected = bounded_strings(&request, "calendar_ids", 50)?;
            let mut settings = state().settings.clone();
            let a = settings.accounts.iter_mut().find(|a| a.id == id).unwrap();
            if selected
                .iter()
                .any(|id| id != "primary" && !a.calendars.iter().any(|c| &c.id == id))
            {
                return Err("A selected calendar is no longer available. Refresh first.".into());
            }
            let mut unique = selected;
            unique.sort();
            unique.dedup();
            a.selected = unique;
            persist(&app, &settings)?;
            let mut s = state();
            s.settings = settings;
            s.events.retain(|e| e.account_id != id);
            drop(s);
            drop(_lock);
            start_sync(app.clone())?;
        }
        "disconnect" => {
            let _lock = WORK
                .try_lock()
                .map_err(|_| "Wait for calendar activity to finish or cancel sign-in.")?;
            let id = account_id(&request)?;
            let mut settings = state().settings.clone();
            settings.accounts.retain(|a| a.id != id);
            // Keep the account available for retry if Keychain denies removal.
            let key=id.clone();
            tokio::task::spawn_blocking(move || oauth::remove(&key))
                .await
                .map_err(|_| "Could not remove Google connection.")??;
            {
                let mut s = state();
                s.tokens.remove(&id);
                s.events.retain(|e| e.account_id != id);
                s.errors.insert(id.clone(),"Local credentials removed. Reconnect or retry removing this account.".into());
            }
            publish(&app);
            persist(&app, &settings)?;
            state().settings=settings;
            state().errors.remove(&id);
        }
        "detection_configure" => {
            let _lock = WORK
                .try_lock()
                .map_err(|_| "Wait for calendar activity to finish.")?;
            let enabled = request["enabled"]
                .as_bool()
                .ok_or("Choose whether detection is enabled.")?;
            let mut s = state();
            let mut settings = s.settings.clone();
            settings.detection_enabled = enabled;
            persist(&app, &settings)?;
            s.settings = settings;
            s.detector = Detector::default();
            s.signals.clear();
            s.detection_error = None;
        }
        "dismiss" => {
            state().detector.dismiss(text(&request, "signal_id")?)?;
            state()
                .signals
                .retain(|s| s.id != request["signal_id"].as_str().unwrap_or(""));
        }
        _ => return Err("Unknown calendar action.".into()),
    }
    publish(&app);
    Ok(snapshot())
}
#[tauri::command]
pub async fn cloud_calendar_link(
    app: AppHandle,
    id: String,
    expected_version: u64,
    event_key: Option<String>,
    use_details: bool,
) -> Result<Meeting, String> {
    load(&app)?;
    let event = match event_key {
        Some(key) => Some(
            state()
                .events
                .iter()
                .find(|e| e.key == key)
                .cloned()
                .ok_or("This event is no longer available. Refresh your calendars.")?,
        ),
        None => None,
    };
    super::operations::cloud_edit(
        app,
        id,
        expected_version,
        Edit::Calendar { event, use_details },
    )
    .await
}
pub fn initialize(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = load(&app) {
            failure(&app, Some(e));
            return;
        }
        let mut ticks = 0u64;
        loop {
            if state().settings.detection_enabled {
                let result = tokio::task::spawn_blocking(detection::sample)
                    .await
                    .unwrap_or_else(|_| Err("Could not inspect call activity.".into()));
                let new_signals = {
                    let mut s = state();
                    let before = s.signals.iter().map(|v| v.id.clone()).collect::<Vec<_>>();
                    if s.settings.detection_enabled {
                        match result {
                            Ok(signals) => {
                                s.signals = s.detector.update(signals);
                                s.detection_error = None;
                            }
                            Err(e) => {
                                s.signals.clear();
                                s.detection_error = Some(e);
                            }
                        }
                    }
                    s.last_scan = Some(chrono::Utc::now().to_rfc3339());
                    s.signals
                        .iter()
                        .filter(|signal| !before.contains(&signal.id))
                        .cloned()
                        .collect::<Vec<_>>()
                };
                publish(&app);
                if !super::active() {
                    for signal in new_signals {
                        use tauri_plugin_notification::NotificationExt;
                        // Uses the existing OS grant. No permission request and no recording starts here.
                        let _ = app
                            .notification()
                            .builder()
                            .title("Record this call?")
                            .body(format!(
                                "{}. Open PartyLine to start recording.",
                                signal.evidence
                            ))
                            .show();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.request_user_attention(Some(
                                tauri::UserAttentionType::Informational,
                            ));
                        }
                    }
                }
            }
            if ticks % 60 == 0 && !state().settings.accounts.is_empty() {
                let _ = start_sync(app.clone());
            }
            ticks = ticks.wrapping_add(1);
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[tokio::test]
    async fn google_pagination_and_http_errors_keep_secrets_out_of_results() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for i in 0..3 {
                let (mut s, _) = listener.accept().await.unwrap();
                let mut b = [0; 8192];
                let n = s.read(&mut b).await.unwrap();
                let r = String::from_utf8_lossy(&b[..n]);
                assert!(r.contains("Bearer fixture-token"));
                if i == 1 {
                    assert!(r.contains("pageToken=next"));
                }
                let body = match i {
                    0 => r#"{"items":[{"id":"one"}],"nextPageToken":"next"}"#,
                    1 => r#"{"items":[{"id":"two"}]}"#,
                    _ => "private-provider-error",
                };
                let code = if i == 2 { "401 Unauthorized" } else { "200 OK" };
                let reply=format!("HTTP/1.1 {code}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
                s.write_all(reply.as_bytes()).await.unwrap();
            }
        });
        let url = url::Url::parse(&format!("http://{addr}/events?maxResults=250")).unwrap();
        let result = pages(url.clone(), "fixture-token").await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[1]["id"], "two");
        let error = get_json(url, "fixture-token").await.unwrap_err();
        assert!(error.contains("Reconnect"));
        assert!(!error.contains("private"));
        assert!(!error.contains("fixture-token"));
        server.await.unwrap();
    }
    #[test]
    fn one_account_failure_never_keeps_stale_events_or_clears_another_account() {
        let mut s = State::default();
        s.settings.accounts = vec![
            Account {
                id: "one".into(),
                ..Default::default()
            },
            Account {
                id: "two".into(),
                ..Default::default()
            },
        ];
        let raw = json!({"id":"same","summary":"Meeting","start":{"dateTime":"2026-11-17T15:00:00-05:00"},"end":{"dateTime":"2026-11-17T16:00:00-05:00"}});
        let one = library::calendar_model::event("one", "primary", &raw).unwrap();
        let two = library::calendar_model::event("two", "primary", &raw).unwrap();
        apply_sync_result(&mut s, "one", Ok((vec![], vec![one.clone()])));
        apply_sync_result(&mut s, "two", Ok((vec![], vec![two.clone()])));
        assert_eq!(s.events.len(), 2);
        apply_sync_result(&mut s, "one", Err("Reconnect".into()));
        assert_eq!(s.events, vec![two]);
        assert_eq!(s.errors["one"], "Reconnect");
        apply_sync_result(&mut s, "one", Ok((vec![], vec![one])));
        assert_eq!(s.events.len(), 2);
        assert!(!s.errors.contains_key("one"));
    }
}
