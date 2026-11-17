//! Provider-independent calendar and detection rules; never handles audio.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub key: String,
    pub account_id: String,
    pub calendar_id: String,
    pub event_id: String,
    pub title: String,
    pub start: String,
    pub end: String,
    pub participants: Vec<String>,
    pub meeting_url: Option<String>,
    pub all_day: bool,
}
impl Event {
    pub fn validate(&self) -> Result<(), String> {
        if self.key.len() > 4096
            || self.account_id.len() > 128
            || self.account_id.is_empty()
            || self.calendar_id.len() > 1024
            || self.event_id.len() > 1024
            || self.title.trim().is_empty()
            || self.title.chars().count() > 200
            || self.participants.len() > 100
            || self
                .participants
                .iter()
                .any(|s| s.is_empty() || s.chars().count() > 80)
        {
            return Err("This calendar event has invalid details. Refresh the calendar.".into());
        }
        let start = chrono::DateTime::parse_from_rfc3339(&self.start)
            .map_err(|_| "Invalid event start.")?;
        let end =
            chrono::DateTime::parse_from_rfc3339(&self.end).map_err(|_| "Invalid event end.")?;
        if end <= start {
            return Err("Invalid event duration.".into());
        }
        if self
            .meeting_url
            .as_ref()
            .is_some_and(|u| meeting_url(u).is_none())
        {
            return Err("Unsupported meeting link.".into());
        }
        Ok(())
    }
}
// Exact hosts, never a substring match or arbitrary URL supplied by an invite.
pub fn meeting_url(raw: &str) -> Option<String> {
    let u = url::Url::parse(raw).ok()?;
    let h = u.host_str()?;
    let allowed = h == "meet.google.com"
        || h == "teams.microsoft.com"
        || h == "teams.live.com"
        || h == "zoom.us"
        || h.ends_with(".zoom.us")
        || h == "zoom.com"
        || h.ends_with(".zoom.com")
        || h == "webex.com"
        || h.ends_with(".webex.com");
    (allowed
        && u.scheme() == "https"
        && u.username().is_empty()
        && u.password().is_none()
        && u.port().is_none()
        && u.path() != "/")
        .then(|| u.to_string())
}
pub fn event(account: &str, calendar: &str, v: &Value) -> Option<Event> {
    if v["status"] == "cancelled"
        || v["attendees"].as_array().is_some_and(|a| {
            a.iter()
                .any(|p| p["self"] == true && p["responseStatus"] == "declined")
        })
    {
        return None;
    }
    // All-day items have no call time and are intentionally excluded from meeting suggestions.
    let start = v["start"]["dateTime"].as_str()?.to_owned();
    let end = v["end"]["dateTime"].as_str()?.to_owned();
    let eid = v["id"].as_str()?;
    let url = v["hangoutLink"]
        .as_str()
        .and_then(meeting_url)
        .or_else(|| {
            v["conferenceData"]["entryPoints"]
                .as_array()?
                .iter()
                .filter(|e| e["entryPointType"] == "video")
                .find_map(|e| e["uri"].as_str().and_then(meeting_url))
        })
        .or_else(|| v["location"].as_str().and_then(meeting_url));
    let participants = v["attendees"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["resource"] != true && p["responseStatus"] != "declined")
        .filter_map(|p| p["displayName"].as_str().or_else(|| p["email"].as_str()))
        .map(|s| s.trim().chars().take(80).collect::<String>())
        .filter(|s| !s.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(100)
        .collect();
    let e = Event {
        key: serde_json::to_string(&(account, calendar, eid, &start)).ok()?,
        account_id: account.into(),
        calendar_id: calendar.into(),
        event_id: eid.into(),
        title: v["summary"]
            .as_str()
            .unwrap_or("Untitled event")
            .trim()
            .chars()
            .take(200)
            .collect(),
        start,
        end,
        participants,
        meeting_url: url,
        all_day: false,
    };
    e.validate().ok()?;
    Some(e)
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Calendar {
    pub id: String,
    pub name: String,
    pub primary: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub email: String,
    pub selected: Vec<String>,
    #[serde(default)]
    pub calendars: Vec<Calendar>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Settings {
    pub client_id: String,
    pub accounts: Vec<Account>,
    pub detection_enabled: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Signal {
    pub id: String,
    pub app: String,
    pub evidence: String,
}
// Browser microphone use cannot identify the web page; never claim Google Meet solely from Chrome.
pub fn classify(bundle: &str) -> Option<(&'static str, &'static str)> {
    let b = bundle.to_lowercase();
    if b == "us.zoom.xos" || b.starts_with("us.zoom.xos.") {
        Some(("Zoom", "Zoom is using the microphone"))
    } else if b == "com.microsoft.teams" || b.starts_with("com.microsoft.teams2") {
        Some(("Microsoft Teams", "Teams is using the microphone"))
    } else if b == "com.tinyspeck.slackmacgap" || b.starts_with("com.tinyspeck.slackmacgap.") {
        Some(("Slack", "Slack is using the microphone"))
    } else if b == "com.apple.facetime" {
        Some(("FaceTime", "FaceTime is using the microphone"))
    } else if b.starts_with("com.cisco.webex") {
        Some(("Webex", "Webex is using the microphone"))
    } else if b == "com.google.chrome" || b.starts_with("com.google.chrome.helper") {
        Some((
            "Chrome",
            "Chrome is using the microphone; this may be a call",
        ))
    } else if b == "com.apple.safari" || b.starts_with("com.apple.webkit") {
        Some((
            "Safari",
            "Safari is using the microphone; this may be a call",
        ))
    } else if b == "com.microsoft.edgemac" || b.starts_with("com.microsoft.edgemac.helper") {
        Some(("Edge", "Edge is using the microphone; this may be a call"))
    } else if b == "org.mozilla.firefox" || b.starts_with("org.mozilla.firefox.") {
        Some((
            "Firefox",
            "Firefox is using the microphone; this may be a call",
        ))
    } else {
        None
    }
}
#[derive(Default)]
pub struct Detector {
    seen: BTreeMap<String, (u32, u32)>,
    dismissed: BTreeSet<String>,
}
impl Detector {
    pub fn dismiss(&mut self, id: &str) -> Result<(), String> {
        if !self.seen.contains_key(id) {
            return Err("This call suggestion is no longer available.".into());
        }
        self.dismissed.insert(id.into());
        Ok(())
    }
    pub fn update(&mut self, signals: Vec<Signal>) -> Vec<Signal> {
        let present: BTreeSet<_> = signals.iter().map(|s| s.id.clone()).collect();
        self.seen.retain(|id, (_, misses)| {
            if !present.contains(id) {
                *misses += 1;
            }
            let keep = *misses < 6;
            if !keep {
                self.dismissed.remove(id);
            }
            keep
        });
        signals
            .into_iter()
            .filter(|s| {
                let count = self.seen.entry(s.id.clone()).or_default();
                count.0 += 1;
                count.1 = 0;
                count.0 >= 2 && !self.dismissed.contains(&s.id)
            })
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn rejects_impostor_links() {
        assert!(meeting_url("https://zoom.us.attacker.test/j/123").is_none());
        assert!(meeting_url("https://attacker@zoom.us/j/123").is_none());
        assert!(meeting_url("https://us02web.zoom.us/j/123").is_some());
    }
    #[test]
    fn accounts_and_occurrences_are_distinct() {
        let v = json!({"id":"same","summary":"Workshop","start":{"dateTime":"2026-11-17T15:00:00-05:00"},"end":{"dateTime":"2026-11-17T16:00:00-05:00"},"attendees":[{"email":"a@test"},{"email":"a@test"},{"email":"no@test","responseStatus":"declined"}]});
        let a = event("a", "primary", &v).unwrap();
        let b = event("b", "primary", &v).unwrap();
        assert_ne!(a.key, b.key);
        assert_eq!(a.participants, vec!["a@test"]);
        let mut declined = v;
        declined["attendees"] = json!([{"self":true,"responseStatus":"declined"}]);
        assert!(event("a", "primary", &declined).is_none());
    }
    #[test]
    fn detection_debounces_dismisses_and_rearms() {
        let mut d = Detector::default();
        let signal = || {
            vec![Signal {
                id: "Zoom".into(),
                app: "Zoom".into(),
                evidence: "microphone".into(),
            }]
        };
        assert!(d.update(signal()).is_empty());
        assert_eq!(d.update(signal()).len(), 1);
        d.dismiss("Zoom").unwrap();
        assert!(d.update(signal()).is_empty());
        for _ in 0..6 {
            d.update(vec![]);
        }
        assert!(d.update(signal()).is_empty());
        assert_eq!(d.update(signal()).len(), 1);
        assert!(classify("com.heychad.partyline").is_none());
        assert!(classify("com.google.Chrome").unwrap().1.contains("may be"));
    }
}
