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
pub struct TimeRange {
    pub id: String,
    pub start: f64,
    pub end: f64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CaptureHealth {
    pub recording: bool,
    pub durable_seconds: f64,
    pub seconds: [f64; 2],
    pub live_skipped_seconds: [f64; 2],
    pub levels: [f32; 2],
    pub received: [bool; 2],
    pub silent: [bool; 2],
    pub updated_at: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Recovery {
    pub status: String,
    pub phase: String,
    pub channel: String,
    pub error: Option<String>,
    pub progress: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Meeting {
    #[serde(default)]
    pub calendar_event: Option<super::calendar_model::Event>,
    #[serde(default)]
    pub speaker_aliases: BTreeMap<String,String>,
    #[serde(default)]
    pub previous_aliases: Option<BTreeMap<String,String>>,
    #[serde(default)]
    pub speaker_edit_version: u64,
    #[serde(default)]
    pub recovery: Recovery,
    #[serde(default)]
    pub capture: CaptureHealth,
    #[serde(default)]
    pub recovery_needed: bool,
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
    #[serde(default)]
    pub edit_version: u64,
    #[serde(default)]
    pub undo_version: Option<u64>,
    #[serde(default)]
    pub trim_start: f64,
    #[serde(default)]
    pub trim_end: Option<f64>,
    #[serde(default)]
    pub redactions: Vec<TimeRange>,
    #[serde(default)]
    pub deleted_at: Option<String>,
    #[serde(default)]
    pub notes_need_review: bool,
}
impl Meeting {
    pub fn new(title: String, participants: Vec<String>) -> Self {
        Self {
            calendar_event: None,
            speaker_aliases: BTreeMap::new(),previous_aliases: None,speaker_edit_version: 0,
            recovery: Recovery::default(),
            capture: CaptureHealth::default(),
            recovery_needed: false,
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
            edit_version: 0,
            undo_version: None,
            trim_start: 0.0,
            trim_end: None,
            redactions: Vec::new(),
            deleted_at: None,
            notes_need_review: false,
        }
    }
    pub fn display_name(&self, speaker: &str) -> String {
        let speaker = self.canonical_speaker(speaker);
        self.names.get(speaker).cloned().unwrap_or_else(|| {
            if speaker == "me" {
                "ME".into()
            } else if speaker.ends_with(":PENDING") || speaker.ends_with(":UNKNOWN") {
                "Identifying speaker".into()
            } else {
                let parts: Vec<_> = speaker.split(':').collect();
                if parts.len() == 3 && parts[1].starts_with("recovered-") { return format!("Recovered speaker {}",parts[2]); }
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
    pub fn canonical_speaker<'a>(&'a self,speaker:&'a str)->&'a str {self.speaker_aliases.get(speaker).map(String::as_str).unwrap_or(speaker)}
    pub fn merge_speakers(&mut self,sources:&[String],target:&str,expected:u64)->Result<(),String>{
        if self.speaker_edit_version!=expected {return Err("Speaker assignments changed. Reload before merging.".into());}
        if self.deleted_at.is_some() || self.recovery.status=="running" {return Err("Restore the recording or wait for recovery before merging.".into());}
        let mut known=std::collections::BTreeSet::from(["me".to_owned()]);
        for t in self.turns.values(){known.insert(self.canonical_speaker(&t.speaker).to_owned());for w in &t.words {known.insert(self.canonical_speaker(&w.speaker).to_owned());}}
        if sources.is_empty() || sources.len()>100 || !known.contains(target) || sources.iter().any(|s|s=="me"||s==target||!known.contains(s)||s.ends_with(":PENDING")||s.ends_with(":UNKNOWN")) || target.ends_with(":PENDING") || target.ends_with(":UNKNOWN") {return Err("Choose distinct identified speakers and a destination speaker. ME can only be the destination.".into());}
        self.previous_aliases=Some(self.speaker_aliases.clone());
        for value in self.speaker_aliases.values_mut(){if sources.contains(value){*value=target.into();}}
        for source in sources {self.speaker_aliases.insert(source.clone(),target.into());}
        self.speaker_edit_version+=1;self.revision+=1;self.speaker_revision+=1;self.notes_need_review=true;Ok(())
    }
    pub fn undo_merge(&mut self,expected:u64)->Result<(),String>{
        if self.speaker_edit_version!=expected || self.deleted_at.is_some() || self.recovery.status=="running" {return Err("Speaker assignments changed or recovery is running. Reload before undoing.".into());}
        self.speaker_aliases=self.previous_aliases.take().ok_or("There is no speaker merge to undo.")?;
        self.speaker_edit_version+=1;self.revision+=1;self.speaker_revision+=1;self.notes_need_review=true;Ok(())
    }
    pub fn ordered_turns(&self) -> Vec<&Turn> {
        let mut turns: Vec<_> = self.turns.values().collect();
        turns.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.id.cmp(&b.id)));
        turns
    }
    pub fn included(&self, start: f64, end: f64) -> bool {
        start >= self.trim_start && end <= self.trim_end.unwrap_or(f64::MAX)
            && !self.redactions.iter().any(|r| start < r.end && end > r.start)
    }
    pub fn turn_transcript(&self, t: &Turn) -> String {
        if !t.final_text { return String::new(); }
        if t.words.is_empty() {
            return if self.included(t.start,t.end) { format!("[{}] {}: {}",stamp(t.start),self.display_name(&t.speaker),t.text) } else { String::new() };
        }
        let mut lines=Vec::new();let mut speaker=String::new();let mut text=String::new();let mut start=t.start;
        for word in &t.words {
            if !self.included(word.start,word.end) {
                if !text.is_empty(){lines.push(format!("[{}] {}: {}",stamp(start),self.display_name(&speaker),text));text.clear();}
                speaker.clear();continue;
            }
            if self.canonical_speaker(&word.speaker)!=speaker {
                if !text.is_empty(){lines.push(format!("[{}] {}: {}",stamp(start),self.display_name(&speaker),text));text.clear();}
                speaker=self.canonical_speaker(&word.speaker).to_owned();start=word.start;
            }
            if !text.is_empty(){text.push(' ');}text.push_str(&word.text);
        }
        if !text.is_empty(){lines.push(format!("[{}] {}: {}",stamp(start),self.display_name(&speaker),text));}
        lines.join("\n")
    }
    pub fn transcript(&self) -> String {
        self.ordered_turns().into_iter().map(|t|self.turn_transcript(t)).filter(|text|!text.is_empty()).collect::<Vec<_>>().join("\n")
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct WorkspacePreferences {
    pub notes_seconds:u64,pub sidebar_width:u32,pub transcript_percent:u32,pub chat_width:u32,
    pub sidebar_visible:bool,pub transcript_visible:bool,pub notes_visible:bool,pub chat_visible:bool,pub pin_transcript:bool,
    pub collapsed_notes:Vec<String>,
}
impl Default for WorkspacePreferences {fn default()->Self{Self{notes_seconds:120,sidebar_width:225,transcript_percent:50,chat_width:360,sidebar_visible:true,transcript_visible:true,notes_visible:true,chat_visible:false,pin_transcript:true,collapsed_notes:vec![]}}}
impl WorkspacePreferences {
    pub fn validate(&self)->Result<(),String>{
        if ![0,30,60,120,300].contains(&self.notes_seconds) || !(150..=400).contains(&self.sidebar_width) || !(20..=80).contains(&self.transcript_percent) || !(280..=700).contains(&self.chat_width) || self.collapsed_notes.len()>500 || self.collapsed_notes.iter().any(|s|s.len()>500) {return Err("Choose valid panel sizes and a supported notes refresh interval.".into());}Ok(())
    }
    pub fn patched(&self,patch:Value)->Result<Self,String>{let object=patch.as_object().ok_or("Settings must be an object.")?;let mut current=serde_json::to_value(self).map_err(|_|"Invalid settings.")?;for (k,v) in object {current[k]=v.clone();}let next:Self=serde_json::from_value(current).map_err(|_|"Unknown or invalid setting.")?;next.validate()?;Ok(next)}
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OutlineState {
    pub default_collapsed: bool,
    // Values are expanded states. The default also applies to newly added nodes.
    pub overrides: std::collections::BTreeMap<String, bool>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag="operation", rename_all="snake_case", deny_unknown_fields)]
pub enum OutlineChange {
    CollapseAll,
    ExpandAll,
    Set { node_id: String, expanded: bool },
}
impl OutlineState {
    pub fn from_legacy(ids: &[String]) -> Self {
        let mut state = Self::default();
        for id in ids {
            let parts: Vec<_> = id.rsplit('/').take(2).collect();
            if parts.len() != 2 || parts[0].parse::<usize>().is_err() { continue; }
            let label: String = parts[1].to_lowercase().chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
            let label = label.split_whitespace().collect::<Vec<_>>().join(" ");
            state.overrides.insert(format!("section/{}/{}", label, parts[0]), false);
        }
        state
    }

    pub fn changed(&self, change: OutlineChange) -> Result<Self, String> {
        let mut next = self.clone();
        match change {
            OutlineChange::CollapseAll => { next.default_collapsed = true; next.overrides.clear(); }
            OutlineChange::ExpandAll => { next.default_collapsed = false; next.overrides.clear(); }
            OutlineChange::Set { node_id, expanded } => {
                if node_id.is_empty() || node_id.len() > 500 { return Err("Invalid outline section.".into()); }
                if expanded == !next.default_collapsed { next.overrides.remove(&node_id); }
                else { next.overrides.insert(node_id, expanded); }
                if next.overrides.len() > 500 { return Err("Too many outline overrides. Use Expand all or Collapse all to reset them.".into()); }
            }
        }
        Ok(next)
    }
}
