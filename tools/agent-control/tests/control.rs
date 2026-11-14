use async_trait::async_trait;
use partyline_agent_control_check::{
    tools, valid, Access, Activity, Backend, Request, Service, MAX_REQUEST_BYTES,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Fake {
    calls: Mutex<Vec<(String, Value)>>,
    activity: Mutex<Vec<Activity>>,
    response: Mutex<Option<Result<Value, String>>>,
}
#[async_trait]
impl Backend for Fake {
    async fn call(&self, tool: &str, arguments: Value) -> Result<Value, String> {
        self.calls.lock().unwrap().push((tool.into(), arguments));
        self.response
            .lock()
            .unwrap()
            .clone()
            .unwrap_or(Ok(json!({"ok":true})))
    }
    fn activity(&self, entry: &Activity) {
        self.activity.lock().unwrap().push(entry.clone());
    }
}
fn request(tool: &str, args: Value) -> Request {
    Request {
        tool: format!("partyline_{tool}"),
        arguments: args,
    }
}

#[tokio::test]
async fn starts_off_and_agents_cannot_grant_access() {
    let backend = Arc::new(Fake::default());
    let service = Service::new(backend.clone());
    assert!(service
        .execute(request("status", json!({"action":"get"})))
        .await
        .is_err());
    assert!(service
        .execute(request(
            "status",
            json!({"action":"set_permissions","access":"control"})
        ))
        .await
        .is_err());
    assert!(backend.calls.lock().unwrap().is_empty());
    assert_eq!(service.activity()[0].outcome, "denied");
}

#[tokio::test]
async fn saved_access_survives_restart_and_still_enforces_permissions() {
    use partyline_agent_control_check::access_store::AccessStore;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("agent-control");
    let store = AccessStore::new(&path).unwrap();
    assert_eq!(store.load().unwrap(), Access::Off);
    for access in [Access::Control, Access::Read, Access::Off] {
        let first = Service::new(Arc::new(Fake::default()));
        store.apply(&first, access).unwrap();
        assert_eq!(first.access(), access);
        // Closing a process clears its in-memory grant, not the saved choice.
        first.set_access(Access::Off);
        let restarted = Service::new(Arc::new(Fake::default()));
        AccessStore::new(&path).unwrap().restore(&restarted).unwrap();
        assert_eq!(restarted.access(), access);
        assert_eq!(restarted.execute(request("status", json!({"action":"get"}))).await.is_ok(), access != Access::Off);
        assert_eq!(restarted.execute(request("status", json!({"action":"show"}))).await.is_ok(), access == Access::Control);
        assert!(restarted.execute(request("status", json!({"action":"set_permissions","access":"control"}))).await.is_err());
    }
}

#[test]
fn malformed_saved_access_fails_closed_and_can_be_repaired() {
    use partyline_agent_control_check::access_store::AccessStore;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("agent-control");
    let store = AccessStore::new(&path).unwrap();
    let service = Service::new(Arc::new(Fake::default()));
    store.apply(&service, Access::Control).unwrap();
    for contents in ["broken", r#"{"version":1,"access":"admin"}"#, r#"{"version":2,"access":"control"}"#, r#"{"version":1,"access":"control","unexpected":true}"#] {
        std::fs::write(path.join("access.json"), contents).unwrap();
        service.set_access(Access::Control);
        assert!(store.restore(&service).is_err());
        assert_eq!(service.access(), Access::Off);
    }
    store.apply(&service, Access::Read).unwrap();
    assert_eq!(store.load().unwrap(), Access::Read);
}

#[cfg(unix)]
#[test]
fn access_settings_are_private_and_reject_symlinks_without_changing_grants() {
    use partyline_agent_control_check::access_store::AccessStore;
    use std::os::unix::fs::{symlink, PermissionsExt};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("agent-control");
    let store = AccessStore::new(&path).unwrap();
    let service = Service::new(Arc::new(Fake::default()));
    store.apply(&service, Access::Read).unwrap();
    let file = path.join("access.json");
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(store.load().is_err());
    // An explicit choice repairs an accidentally broadened file mode.
    store.apply(&service, Access::Read).unwrap();
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
    std::fs::remove_file(&file).unwrap();
    let unrelated = root.path().join("unrelated");
    std::fs::write(&unrelated, "unchanged").unwrap();
    symlink(&unrelated, &file).unwrap();
    assert!(store.apply(&service, Access::Control).is_err());
    assert_eq!(service.access(), Access::Read);
    assert_eq!(std::fs::read_to_string(&unrelated).unwrap(), "unchanged");
    assert!(store.restore(&service).is_err());
    assert_eq!(service.access(), Access::Off);
    let link = root.path().join("linked-directory");
    symlink(&path, &link).unwrap();
    assert!(AccessStore::new(&link).is_err());
}

fn sample(schema: &Value) -> Value {
    if let Some(value) = schema.get("const") {
        return value.clone();
    }
    if let Some(options) = schema["enum"].as_array() {
        return options[0].clone();
    }
    match schema["type"].as_str().unwrap_or("") {
        "string" if schema["format"] == "uuid" => json!("a1a1a1a1-1111-4111-8111-111111111111"),
        "string" => json!("test"),
        "boolean" => json!(true),
        "integer" => schema.get("minimum").cloned().unwrap_or(json!(0)),
        "array" => json!([]),
        "object" => {
            let mut value = json!({});
            for name in schema["required"].as_array().unwrap() {
                let name = name.as_str().unwrap();
                value[name] = sample(&schema["properties"][name]);
            }
            value
        }
        _ => panic!("unhandled schema"),
    }
}

#[tokio::test]
async fn every_action_obeys_its_permission_and_rejects_unknown_arguments() {
    let backend = Arc::new(Fake::default());
    let service = Service::new(backend.clone());
    service.set_access(Access::Read);
    assert_eq!(tools().len(), 8);
    for tool in tools() {
        for schema in tool["inputSchema"]["oneOf"].as_array().unwrap() {
            let mut args = sample(schema);
            assert!(valid(&tool["inputSchema"], &args));
            let action = args["action"].as_str().unwrap();
            let expected = tool["_meta"]["partyline/permissions"][action] == "read";
            let result = service
                .execute(Request {
                    tool: tool["name"].as_str().unwrap().into(),
                    arguments: args.clone(),
                })
                .await;
            assert_eq!(result.is_ok(), expected, "{}/{}", tool["name"], action);
            args["unexpected"] = json!("never-dispatch");
            assert!(!valid(&tool["inputSchema"], &args));
        }
    }
    service.set_access(Access::Off);
    assert!(service
        .execute(request("meetings", json!({"action":"list"})))
        .await
        .is_err());
}

#[tokio::test]
async fn secrets_never_appear_in_results_errors_or_activity() {
    let backend = Arc::new(Fake::default());
    let service = Service::new(backend.clone());
    service.set_access(Access::Control);
    let secret = "sk-TEST-should-never-be-returned";
    *backend.response.lock().unwrap() = Some(Ok(
        json!({"assemblyai":true,"openai":true,"key":secret,"nested":{"token":secret}}),
    ));
    let args = json!({"action":"save","openai":secret});
    let output = service
        .execute(request("connections", args.clone()))
        .await
        .unwrap();
    assert_eq!(output, json!({"assemblyai":true,"openai":true,"ok":true}));
    *backend.response.lock().unwrap() = Some(Err(format!("backend accidentally echoed {secret}")));
    assert!(!service
        .execute(request("connections", args))
        .await
        .unwrap_err()
        .contains(secret));
    let invalid = service
        .execute(request("connections", json!({"action":secret})))
        .await
        .unwrap_err();
    assert!(!invalid.contains(secret));
    assert!(!serde_json::to_string(&service.activity())
        .unwrap()
        .contains(secret));
    assert!(!serde_json::to_string(&*backend.activity.lock().unwrap())
        .unwrap()
        .contains(secret));
}

#[test]
fn identifiers_limits_and_wrong_types_fail_closed() {
    let catalog = tools();
    let schema = &catalog
        .iter()
        .find(|t| t["name"] == "partyline_meetings")
        .unwrap()["inputSchema"];
    for args in [
        json!({"action":"get","id":"../../keys"}),
        json!({"action":"list","limit":0}),
        json!({"action":"list","limit":501}),
        json!({"action":"list","offset":-1}),
        json!({"action":"follow","enabled":"true"}),
        json!({"action":"prepare","title":"x".repeat(201)}),
    ] {
        assert!(!valid(schema, &args));
    }
}

#[tokio::test]
async fn activity_is_bounded_and_contains_no_meeting_content() {
    let backend = Arc::new(Fake::default());
    let service = Service::new(backend);
    service.set_access(Access::Control);
    for _ in 0..120 {
        service
            .execute(request(
                "recording",
                json!({"action":"start","title":"PRIVATE MEETING"}),
            ))
            .await
            .unwrap();
    }
    let activity = service.activity();
    assert_eq!(activity.len(), 100);
    assert_eq!(activity.first().unwrap().sequence, 21);
    assert!(activity.iter().all(|e| e.outcome == "completed"));
    assert!(!serde_json::to_string(&activity)
        .unwrap()
        .contains("PRIVATE MEETING"));
}

struct Blocking {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
    calls: std::sync::atomic::AtomicUsize,
}
#[async_trait]
impl Backend for Blocking {
    async fn call(&self, _: &str, _: Value) -> Result<Value, String> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.entered.notify_one();
        self.release.notified().await;
        Ok(json!({"ok":true}))
    }
}

#[tokio::test]
async fn queued_mutation_is_denied_when_human_revokes_access() {
    let backend = Arc::new(Blocking {
        entered: Default::default(),
        release: Default::default(),
        calls: Default::default(),
    });
    let service = Arc::new(Service::new(backend.clone()));
    service.set_access(Access::Control);
    let one = service.clone();
    let first = tokio::spawn(async move {
        one.execute(request("status", json!({"action":"show"})))
            .await
    });
    backend.entered.notified().await;
    let two = service.clone();
    let second = tokio::spawn(async move {
        two.execute(request("status", json!({"action":"hide"})))
            .await
    });
    tokio::task::yield_now().await;
    service.set_access(Access::Off);
    backend.release.notify_one();
    assert!(first.await.unwrap().is_ok());
    assert!(second.await.unwrap().is_err());
    assert_eq!(backend.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn real_socket_enforces_permissions_frames_and_cleanup() {
    use partyline_agent_control_check::socket::Endpoint;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("a");
    let backend = Arc::new(Fake::default());
    let service = Arc::new(Service::new(backend.clone()));
    service.set_access(Access::Read);
    let endpoint = Endpoint::bind(&directory).unwrap();
    let path = directory.join("control.sock");
    assert_eq!(std::fs::metadata(&directory).unwrap().mode() & 0o777, 0o700);
    assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    assert!(Endpoint::bind(&directory).is_err());
    let (stop, receiver) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(endpoint.serve(service, receiver));
    for (payload, ok) in [
        (
            b"{\"tool\":\"partyline_status\",\"arguments\":{\"action\":\"get\"}}\n".to_vec(),
            true,
        ),
        (
            b"{\"tool\":\"partyline_recording\",\"arguments\":{\"action\":\"start\"}}\n".to_vec(),
            false,
        ),
        (b"not-json\n".to_vec(), false),
        (vec![b'x'; MAX_REQUEST_BYTES + 1], false),
    ] {
        let mut client = tokio::net::UnixStream::connect(&path).await.unwrap();
        client.write_all(&payload).await.unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        let result: Value = serde_json::from_slice(&response).unwrap();
        assert_eq!(result["ok"], ok);
    }
    stop.send(true).unwrap();
    server.await.unwrap();
    assert!(!path.exists());
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(Endpoint::bind(&directory).is_err());
    assert_eq!(backend.calls.lock().unwrap().len(), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn refuses_symlink_and_recovers_only_a_stale_socket() {
    use partyline_agent_control_check::socket::Endpoint;
    use std::os::unix::{
        fs::{symlink, PermissionsExt},
        net::UnixListener,
    };
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("a");
    std::fs::create_dir(&directory).unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = directory.join("control.sock");
    let target = root.path().join("secret");
    std::fs::write(&target, "untouched").unwrap();
    symlink(&target, &path).unwrap();
    assert!(Endpoint::bind(&directory).is_err());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "untouched");
    std::fs::remove_file(&path).unwrap();
    drop(UnixListener::bind(&path).unwrap());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(Endpoint::bind(&directory).is_ok());
}

#[cfg(unix)]
#[tokio::test]
async fn mcp_stdio_reaches_the_actual_permissioned_service() {
    use partyline_agent_control_check::socket::Endpoint;
    use tokio::io::AsyncWriteExt;
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("a");
    let backend = Arc::new(Fake::default());
    let service = Arc::new(Service::new(backend.clone()));
    service.set_access(Access::Read);
    let endpoint = Endpoint::bind(&directory).unwrap();
    let (stop, receiver) = tokio::sync::watch::channel(false);
    let host = tokio::spawn(endpoint.serve(service, receiver));
    let mut bridge = tokio::process::Command::new("python3")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/server.py"))
        .arg("--socket")
        .arg(directory.join("control.sock"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let messages = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"partyline_status","arguments":{"action":"get"}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"partyline_recording","arguments":{"action":"start"}}}),
    ];
    let mut input = bridge.stdin.take().unwrap();
    for message in messages {
        input
            .write_all(format!("{message}\n").as_bytes())
            .await
            .unwrap();
    }
    drop(input);
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        bridge.wait_with_output(),
    )
    .await
    .unwrap()
    .unwrap();
    stop.send(true).unwrap();
    host.await.unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let responses: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(responses.len(), 3);
    assert_eq!(
        responses[1]["result"]["structuredContent"]["agent_access"],
        "read"
    );
    assert_eq!(responses[2]["result"]["isError"], true);
    assert_eq!(backend.calls.lock().unwrap().len(), 1);
}
