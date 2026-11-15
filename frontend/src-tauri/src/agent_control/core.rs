//! Permission checks and transport shared by the app and the isolated test crate.
//! No recording, Keychain, provider, or window code belongs here.
#[cfg(unix)]
#[path = "socket.rs"]
pub mod socket;
#[path = "access_store.rs"]
pub mod access_store;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

pub const MAX_REQUEST_BYTES: usize = 512 * 1024;
pub const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const AUDIT_LIMIT: usize = 100;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    #[default]
    Off = 0,
    Read = 1,
    Control = 2,
}

#[derive(Clone, Debug, Serialize)]
pub struct Activity {
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub tool: String,
    pub action: String,
    pub label: String,
    pub outcome: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub tool: String,
    pub arguments: Value,
}

#[async_trait]
pub trait Backend: Send + Sync {
    async fn call(&self, tool: &str, arguments: Value) -> Result<Value, String>;
    fn activity(&self, _entry: &Activity) {}
}

pub struct Service {
    backend: Arc<dyn Backend>,
    access: AtomicU8,
    mutations: tokio::sync::Mutex<()>,
    audit: Mutex<VecDeque<Activity>>,
    sequence: std::sync::atomic::AtomicU64,
    catalog: Vec<Value>,
}

pub fn tools() -> Vec<Value> {
    serde_json::from_str(include_str!("tools.json")).expect("validated agent tool catalog")
}

impl Service {
    pub fn new(backend: Arc<dyn Backend>) -> Self {
        Self {
            backend,
            access: AtomicU8::new(Access::Off as u8),
            mutations: tokio::sync::Mutex::new(()),
            audit: Mutex::new(VecDeque::new()),
            sequence: std::sync::atomic::AtomicU64::new(0),
            catalog: tools(),
        }
    }

    pub fn access(&self) -> Access {
        match self.access.load(Ordering::SeqCst) {
            1 => Access::Read,
            2 => Access::Control,
            _ => Access::Off,
        }
    }

    // Called only from the app's trusted local UI. There is no remote grant tool.
    pub fn set_access(&self, access: Access) {
        self.access.store(access as u8, Ordering::SeqCst);
    }

    pub fn activity(&self) -> Vec<Activity> {
        self.audit.lock().unwrap().iter().cloned().collect()
    }

    fn record(&self, tool: &str, action: &str, label: &str, outcome: &str, sequence: u64) {
        let entry = Activity {
            sequence,
            timestamp_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            tool: tool.into(),
            action: action.into(),
            label: label.into(),
            outcome: outcome.into(),
        };
        {
            let mut audit = self.audit.lock().unwrap();
            if let Some(existing) = audit.iter_mut().find(|e| e.sequence == sequence) {
                *existing = entry.clone();
            } else {
                audit.push_back(entry.clone());
                if audit.len() > AUDIT_LIMIT {
                    audit.pop_front();
                }
            }
        }
        self.backend.activity(&entry);
    }

    fn allowed(&self, permission: &str) -> bool {
        match self.access() {
            Access::Control => true,
            Access::Read => permission == "read",
            Access::Off => false,
        }
    }

    pub async fn execute(&self, request: Request) -> Result<Value, String> {
        let tool = self
            .catalog
            .iter()
            .find(|t| t["name"] == request.tool)
            .ok_or("Unknown PartyLine tool.")?;
        if !valid(&tool["inputSchema"], &request.arguments) {
            // Never echo the invalid value: this includes secret inputs.
            return Err("Invalid tool arguments. Use the tool's input schema.".into());
        }
        let action = request.arguments["action"]
            .as_str()
            .ok_or("Missing action.")?;
        let permission = tool["_meta"]["partyline/permissions"][action]
            .as_str()
            .ok_or("Unsupported action.")?;
        let label = tool["_meta"]["partyline/labels"][action]
            .as_str()
            .unwrap_or(action);
        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst) + 1;
        if !self.allowed(permission) {
            self.record(&request.tool, action, label, "denied", sequence);
            return Err(
                "Agent access does not allow this action. Change access in PartyLine.".into(),
            );
        }
        // Writes from multiple agents are serialized. Reads remain available during them.
        let _guard = if permission == "control" {
            Some(self.mutations.lock().await)
        } else {
            None
        };
        // A human may revoke access while this request waits for a previous write.
        if !self.allowed(permission) {
            self.record(&request.tool, action, label, "denied", sequence);
            return Err("Agent access was revoked before this action started.".into());
        }
        self.record(&request.tool, action, label, "started", sequence);
        let result = if request.tool == "partyline_status" && action == "activity" {
            Ok(json!({"activity": self.activity()}))
        } else {
            self.backend
                .call(&request.tool, request.arguments.clone())
                .await
        };
        self.record(
            &request.tool,
            action,
            label,
            if result.is_ok() {
                "completed"
            } else {
                "failed"
            },
            sequence,
        );
        // Connection responses are a strict allowlist. No backend error body, supplied
        // credential, or accidental future Keychain field can escape through this tool.
        if request.tool == "partyline_connections" {
            return result.map(|value| {
                json!({
                    "assemblyai": value.get("assemblyai").and_then(Value::as_bool),
                    "openai": value.get("openai").and_then(Value::as_bool),
                    "ok": true,
                })
            }).map_err(|_| "Connection action failed. Check the saved keys, account access, and network in PartyLine.".into());
        }
        result.map(|mut value| {
            if request.tool == "partyline_status" && action == "get" {
                value["agent_access"] = json!(self.access());
            }
            value
        })
    }
}

// The catalog deliberately uses this small, closed JSON Schema subset. Reject
// extra arguments even when a client doesn't validate schemas itself.
pub fn valid(schema: &Value, value: &Value) -> bool {
    if let Some(alternatives) = schema.get("oneOf").and_then(Value::as_array) {
        if alternatives.iter().filter(|s| valid(s, value)).count() != 1 {
            return false;
        }
    }
    if let Some(expected) = schema.get("const") {
        if value != expected {
            return false;
        }
    }
    if let Some(options) = schema.get("enum").and_then(Value::as_array) {
        if !options.contains(value) {
            return false;
        }
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("object") => {
            let Some(object) = value.as_object() else {
                return false;
            };
            if let Some(required) = schema.get("required").and_then(Value::as_array) {
                if required
                    .iter()
                    .any(|key| !object.contains_key(key.as_str().unwrap_or("")))
                {
                    return false;
                }
            }
            if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
                for (key, item) in object {
                    match properties.get(key) {
                        Some(rule) => {
                            if !valid(rule, item) {
                                return false;
                            }
                        }
                        None => {
                            if schema["additionalProperties"] == false {
                                return false;
                            }
                        }
                    }
                }
            }
        }
        Some("string") => {
            let Some(text) = value.as_str() else {
                return false;
            };
            let length = text.chars().count() as u64;
            if schema["maxLength"].as_u64().is_some_and(|n| length > n)
                || schema["minLength"].as_u64().is_some_and(|n| length < n)
            {
                return false;
            }
            if schema["format"] == "uuid" && uuid::Uuid::parse_str(text).is_err() {
                return false;
            }
            if text.contains('\0') {
                return false;
            }
        }
        Some("boolean") => {
            if !value.is_boolean() {
                return false;
            }
        }
        Some("integer") => {
            let Some(n) = value.as_u64() else {
                return false;
            };
            if schema["minimum"].as_u64().is_some_and(|min| n < min)
                || schema["maximum"].as_u64().is_some_and(|max| n > max)
            {
                return false;
            }
        }
        Some("number") => {
            let Some(n) = value.as_f64() else { return false; };
            if !n.is_finite() || schema["minimum"].as_f64().is_some_and(|min| n < min)
                || schema["maximum"].as_f64().is_some_and(|max| n > max) { return false; }
        }
        Some("array") => {
            let Some(array) = value.as_array() else {
                return false;
            };
            if schema["maxItems"]
                .as_u64()
                .is_some_and(|n| array.len() as u64 > n)
                || !array.iter().all(|v| valid(&schema["items"], v))
            {
                return false;
            }
        }
        None => {}
        _ => return false,
    }
    true
}
