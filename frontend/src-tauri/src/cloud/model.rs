use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Word {
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub speaker: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Turn {
    pub id: String,
    pub channel: String,
    pub order: u64,
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub speaker: String,
    pub final_text: bool,
    pub words: Vec<Word>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Meeting {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub status: String,
    pub duration: f64,
    pub turns: BTreeMap<String, Turn>,
    pub names: BTreeMap<String, String>,
    pub participants: Vec<String>,
    pub notes: String,
    #[serde(default)]
    pub notes_draft: String,
    #[serde(default)]
    pub notes_status: String,
    #[serde(default)]
    pub notes_error: Option<String>,
    #[serde(default)]
    pub connection: BTreeMap<String, String>,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub speaker_revision: u64,
    #[serde(default)]
    pub notes_revision: u64,
}
impl Meeting {
    pub fn new(title: String, participants: Vec<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            created_at: chrono::Utc::now().to_rfc3339(),
            status: "connecting".into(),
            duration: 0.0,
            turns: BTreeMap::new(),
            names: BTreeMap::from([("me".into(), "ME".into())]),
            participants,
            notes: String::new(),
            notes_draft: String::new(),
            notes_status: "waiting".into(),
            notes_error: None,
            connection: BTreeMap::new(),
            errors: Vec::new(),
            revision: 0,
            speaker_revision: 0,
            notes_revision: 0,
        }
    }
    pub fn display_name(&self, speaker: &str) -> String {
        self.names.get(speaker).cloned().unwrap_or_else(|| {
            if speaker == "me" {
                "ME".into()
            } else if speaker.ends_with(":PENDING") || speaker.ends_with(":UNKNOWN") {
                "Identifying speaker".into()
            } else {
                let parts: Vec<_> = speaker.split(':').collect();
                if parts.len() == 3 && parts[1] != "0" {
                    format!(
                        "Speaker {} · connection {}",
                        parts[2],
                        parts[1].parse::<u32>().unwrap_or(0) + 1
                    )
                } else {
                    format!("Speaker {}", parts.last().unwrap_or(&speaker))
                }
            }
        })
    }
    pub fn ordered_turns(&self) -> Vec<&Turn> {
        let mut turns: Vec<_> = self.turns.values().collect();
        turns.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.id.cmp(&b.id)));
        turns
    }
    pub fn transcript(&self) -> String {
        self.ordered_turns()
            .into_iter()
            .filter(|t| t.final_text)
            .map(|t| {
                if t.words.is_empty() {
                    return format!(
                        "[{}] {}: {}",
                        stamp(t.start),
                        self.display_name(&t.speaker),
                        t.text
                    );
                }
                let mut lines = Vec::new();
                let mut speaker = String::new();
                let mut text = String::new();
                let mut start = t.start;
                for w in &t.words {
                    if w.speaker != speaker {
                        if !text.is_empty() {
                            lines.push(format!(
                                "[{}] {}: {}",
                                stamp(start),
                                self.display_name(&speaker),
                                text
                            ));
                        }
                        speaker = w.speaker.clone();
                        text.clear();
                        start = w.start;
                    }
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    text.push_str(&w.text);
                }
                if !text.is_empty() {
                    lines.push(format!(
                        "[{}] {}: {}",
                        stamp(start),
                        self.display_name(&speaker),
                        text
                    ));
                }
                lines.join("\n")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn apply(&mut self, channel: &str, generation: u32, offset: f64, event: &Value) -> bool {
        let kind = event["type"].as_str().unwrap_or("");
        if kind == "SpeakerRevision" {
            let mut changed = false;
            if let Some(revisions) = event["revisions"].as_array() {
                for r in revisions {
                    let id = format!(
                        "{channel}:{generation}:{}",
                        r["turn_order"].as_u64().unwrap_or(0)
                    );
                    if let Some(t) = self.turns.get_mut(&id) {
                        t.speaker = speaker_id(channel, generation, r["speaker_label"].as_str());
                        if let Some(words) = r["words"].as_array() {
                            for (old, new) in t.words.iter_mut().zip(words) {
                                old.speaker = speaker_id(
                                    channel,
                                    generation,
                                    new["speaker"].as_str().or(r["speaker_label"].as_str()),
                                );
                            }
                        }
                        changed = true;
                    }
                }
            }
            if changed {
                self.revision += 1;
                self.speaker_revision += 1;
            }
            return changed;
        }
        if kind != "Turn" {
            return false;
        }
        let text = event["transcript"].as_str().unwrap_or("").to_string();
        if text.trim().is_empty() {
            return false;
        }
        let order = event["turn_order"].as_u64().unwrap_or(0);
        let id = format!("{channel}:{generation}:{order}");
        let label = event["speaker_label"].as_str();
        let words: Vec<Word> = event["words"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|w| Word {
                text: w["text"].as_str().unwrap_or("").into(),
                start: offset + w["start"].as_f64().unwrap_or(0.0) / 1000.0,
                end: offset + w["end"].as_f64().unwrap_or(0.0) / 1000.0,
                speaker: speaker_id(channel, generation, w["speaker"].as_str().or(label)),
            })
            .collect();
        let final_text = event["end_of_turn"].as_bool().unwrap_or(false);
        if self
            .turns
            .get(&id)
            .is_some_and(|t| t.final_text && !final_text)
        {
            return false;
        }
        let t = Turn {
            id: id.clone(),
            channel: channel.into(),
            order,
            text,
            start: words.first().map_or(offset, |w| w.start),
            end: words.last().map_or(offset, |w| w.end),
            speaker: speaker_id(channel, generation, label),
            final_text,
            words,
        };
        if let Some(old) = self.turns.get(&id) {
            if old.final_text
                && (old.speaker != t.speaker
                    || old.words.iter().map(|w| &w.speaker).collect::<Vec<_>>()
                        != t.words.iter().map(|w| &w.speaker).collect::<Vec<_>>())
            {
                self.speaker_revision += 1;
            }
        }
        if final_text {
            self.revision += 1;
        }
        self.turns.insert(id, t);
        true
    }
}
pub fn stamp(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    format!("{:02}:{:02}", s / 60, s % 60)
}
fn speaker_id(channel: &str, generation: u32, label: Option<&str>) -> String {
    if channel == "me" {
        "me".into()
    } else {
        format!("remote:{generation}:{}", label.unwrap_or("PENDING"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn partials_replace_and_revisions_preserve_words() {
        let mut m = Meeting::new("Test".into(), vec![]);
        m.apply("remote",0,2.0,&json!({"type":"Turn","turn_order":1,"transcript":"Hello","end_of_turn":false,"words":[{"text":"Hello","start":0,"end":400}]}));
        assert_eq!(m.transcript(), "");
        m.apply("remote",0,2.0,&json!({"type":"Turn","turn_order":1,"transcript":"Hello there","speaker_label":"A","end_of_turn":true,"words":[{"text":"Hello","start":0,"end":400,"speaker":"A"},{"text":"there","start":500,"end":800,"speaker":"B"}]}));
        assert_eq!(m.turns.len(), 1);
        assert!(m.transcript().contains("Speaker B: there"));
        m.apply("remote",0,2.0,&json!({"type":"SpeakerRevision","revisions":[{"turn_order":1,"speaker_label":"B","words":[{"speaker":"B","text":"MUST NOT REPLACE"},{"speaker":"B"}]}]}));
        m.names.insert("remote:0:B".into(), "Nicole".into());
        assert_eq!(m.transcript(), "[00:02] Nicole: Hello there");
        assert_eq!(m.speaker_revision, 1);
    }
    #[test]
    fn mic_identity_and_reconnect_identity_are_separate() {
        let mut m = Meeting::new("Test".into(), vec![]);
        let e = json!({"type":"Turn","turn_order":0,"transcript":"Test","speaker_label":"A","end_of_turn":true});
        m.apply("me", 0, 0.0, &e);
        m.apply("remote", 0, 0.0, &e);
        m.apply("remote", 1, 5.0, &e);
        assert_eq!(m.turns.len(), 3);
        assert_eq!(m.turns["me:0:0"].speaker, "me");
        assert_ne!(m.turns["remote:0:0"].speaker, m.turns["remote:1:0"].speaker);
    }
}
