//! Shared meeting editing, sharing, and audio logic. No UI, providers, or Keychain.
#[path = "model.rs"]
pub mod model;
#[path = "calendar/domain.rs"]
pub mod calendar_model;
use model::{Meeting, TimeRange};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    Calendar { event: Option<calendar_model::Event>, use_details: bool },
    Details { title: String, participants: Vec<String> },
    Transcript { turn_id: String, text: String, start: Option<f64>, end: Option<f64> },
    Notes { text: String },
    Trim { start: f64, end: f64 },
    Redact { start: f64, end: f64 },
    Unredact { redaction_id: String },
    Trash,
    Restore,
    Undo,
}

pub fn idle(m: &Meeting) -> Result<(), String> {
    if m.recovery.status=="running" {return Err("Wait for transcript recovery or cancel it first.".into());}
    if !["complete", "interrupted"].contains(&m.status.as_str()) {
        return Err("Finish the recording before editing or sharing its audio.".into());
    }
    if ["updating", "finalizing"].contains(&m.notes_status.as_str()) {
        return Err("Wait for notes to finish, then try again.".into());
    }
    Ok(())
}

pub fn replace_recovered_channel(m: &mut Meeting, channel: &str, generation: &str, response: &serde_json::Value) -> Result<(), String> {
    if !["me", "remote"].contains(&channel) || response["status"] != "completed" { return Err("The recovered transcript is not complete.".into()); }
    let words = response["words"].as_array().ok_or("Recovery returned no word alignment.")?;
    let mut turns = Vec::new();
    let mut group: Vec<model::Word> = Vec::new();
    for w in words {
        let start = w["start"].as_f64().ok_or("Missing recovered timestamp.")? / 1000.0;
        let end = w["end"].as_f64().ok_or("Missing recovered timestamp.")? / 1000.0;
        if !start.is_finite() || !end.is_finite() || start < 0.0 || end < start || end > m.duration + 1.0 { return Err("Recovered timestamps do not match this recording.".into()); }
        let speaker = if channel == "me" { "me".to_owned() } else { format!("remote:recovered-{generation}:{}", w["speaker"].as_str().unwrap_or("UNKNOWN")) };
        if group.first().is_some_and(|first| start - first.start > 15.0 || first.speaker != speaker) { turns.push(std::mem::take(&mut group)); }
        group.push(model::Word { text: w["text"].as_str().ok_or("Missing recovered text.")?.into(), start, end, speaker });
    }
    if !group.is_empty() { turns.push(group); }
    if turns.is_empty() && m.turns.values().any(|t|t.channel == channel) { return Err("Recovery returned no speech. The existing transcript was kept.".into()); }
    m.turns.retain(|_,t|t.channel != channel);
    for (i, words) in turns.into_iter().enumerate() {
        let id = format!("{channel}:recovered-{generation}:{i}");
        m.turns.insert(id.clone(), model::Turn { id, channel: channel.into(), order: i as u64, text: words.iter().map(|w|w.text.as_str()).collect::<Vec<_>>().join(" "), start: words[0].start, end: words.last().unwrap().end, speaker: words[0].speaker.clone(), final_text: true, words });
    }
    Ok(())
}

fn range(m: &Meeting, start: f64, end: f64) -> Result<(), String> {
    if !start.is_finite() || !end.is_finite() || start < 0.0 || end <= start || end > m.duration {
        Err("Choose start and end times within the recording.".into())
    } else { Ok(()) }
}

pub fn edited(m: &Meeting, expected: u64, edit: &Edit) -> Result<Meeting, String> {
    let live_details = matches!(edit, Edit::Details { .. } | Edit::Calendar { .. }) && ["connecting", "recording", "paused"].contains(&m.status.as_str());
    if !live_details { idle(m)?; }
    if m.edit_version != expected { return Err("This meeting changed. Reload it before saving your edit.".into()); }
    if m.deleted_at.is_some() && !matches!(edit, Edit::Restore) { return Err("Restore this meeting from Trash first.".into()); }
    let mut next = m.clone();
    match edit {
        Edit::Calendar { event, use_details } => {
            if let Some(event)=event { event.validate()?; if *use_details { next.title=event.title.clone(); next.participants=event.participants.clone(); } }
            next.calendar_event=event.clone();
        }
        Edit::Details { title, participants } => {
            if title.trim().is_empty() || title.contains('\0') || title.chars().count() > 200 || participants.len() > 100
                || participants.iter().any(|p| p.trim().is_empty() || p.contains('\0') || p.chars().count() > 80) {
                return Err("Use a title of 1–200 characters and up to 100 participant names of 1–80 characters.".into());
            }
            next.title = title.trim().into();
            next.participants = participants.iter().map(|p| p.trim().to_owned()).collect();
        }
        Edit::Transcript { turn_id, text, start, end } => {
            if text.trim().is_empty() || text.contains('\0') || text.chars().count() > 16000 { return Err("Use 1–16,000 characters for a transcript turn.".into()); }
            let turn = next.turns.get_mut(turn_id).ok_or("Transcript turn not found.")?;
            if !turn.final_text { return Err("Wait for the transcript turn to finish.".into()); }
            match (start,end) {
                (Some(start),Some(end)) if !turn.words.is_empty() => {
                    let first=turn.words.iter().position(|w|(w.start-start).abs()<0.00001).ok_or("The passage changed. Reopen the editor.")?;
                    let last=turn.words.iter().rposition(|w|(w.end-end).abs()<0.00001).ok_or("The passage changed. Reopen the editor.")?;
                    if last<first || turn.words[first..=last].iter().any(|w|w.speaker!=turn.words[first].speaker) {return Err("Edit one speaker passage at a time.".into());}
                    let speaker=turn.words[first].speaker.clone();
                    turn.words.splice(first..=last,[model::Word{text:text.trim().into(),start:*start,end:*end,speaker}]);
                    turn.text=turn.words.iter().map(|w|w.text.as_str()).collect::<Vec<_>>().join(" ");
                }
                (None,None) | (Some(_),Some(_)) => {
                    if turn.words.iter().any(|w|w.speaker!=turn.speaker){return Err("This turn has multiple speakers. Supply the start and end of one passage.".into());}
                    turn.text=text.trim().into();
                    // Retain turn times without inventing alignments for newly edited words.
                    turn.words.clear();
                }
                _ => return Err("Supply both start and end for a passage edit.".into()),
            }
            next.notes_need_review = true;
        }
        Edit::Notes { text } => {
            if text.contains('\0') || text.chars().count() > 50000 { return Err("Keep notes under 50,000 characters.".into()); }
            next.notes = text.clone();
            next.notes_draft.clear();
            next.notes_error = None;
            next.notes_need_review = false;
        }
        Edit::Trim { start, end } => {
            range(m, *start, *end)?;
            next.trim_start = *start;
            next.trim_end = Some(*end);
            next.notes_need_review = true;
        }
        Edit::Redact { start, end } => {
            range(m, *start, *end)?;
            if next.redactions.len() >= 500 { return Err("This recording already has 500 redactions.".into()); }
            next.redactions.push(TimeRange { id: uuid::Uuid::new_v4().to_string(), start: *start, end: *end });
            next.notes_need_review = true;
        }
        Edit::Unredact { redaction_id } => {
            let count = next.redactions.len();
            next.redactions.retain(|r| r.id != *redaction_id);
            if count == next.redactions.len() { return Err("Redaction not found.".into()); }
            next.notes_need_review = true;
        }
        Edit::Trash => { next.deleted_at = Some(chrono::Utc::now().to_rfc3339()); }
        Edit::Restore => {
            if next.deleted_at.is_none() { return Err("This meeting is not in Trash.".into()); }
            next.deleted_at = None;
        }
        Edit::Undo => return Err("Undo requires the saved revision.".into()),
    }
    next.edit_version += 1;
    next.revision += 1;
    if matches!(edit, Edit::Notes { .. }) || (matches!(edit, Edit::Trash | Edit::Restore) && m.notes_revision == m.revision) { next.notes_revision = next.revision; }
    // A whole-meeting undo of a live snapshot would erase later transcript/audio state.
    // Live metadata remains versioned, but cannot restore that obsolete snapshot.
    next.undo_version = if live_details { None } else { Some(m.edit_version) };
    Ok(next)
}

pub fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let parent = path.parent().ok_or("Invalid data folder.")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|_| "Could not create a save file. Check disk space.")?;
    serde_json::to_writer_pretty(&mut temp, value).map_err(|_| "Could not write the save file.")?;
    temp.flush().map_err(|_| "Could not finish writing the save file.")?;
    temp.as_file().sync_all().map_err(|_| "Could not save the file to disk.")?;
    temp.persist(path).map_err(|_| "Could not replace the saved file.")?;
    Ok(())
}
pub fn atomic_text(path: &Path, text: &str) -> Result<(), String> {
    let mut temp=tempfile::NamedTempFile::new_in(path.parent().ok_or("Invalid output folder.")?).map_err(|_|"Could not create an output file.")?;
    temp.write_all(text.as_bytes()).map_err(|_|"Could not write the output file.")?;
    temp.as_file().sync_all().map_err(|_|"Could not save the output file.")?;
    temp.persist(path).map_err(|_|"Could not replace the output file.")?;Ok(())
}

pub fn save_edit(folder: &Path, current: &Meeting, expected: u64, edit: &Edit) -> Result<Meeting, String> {
    let next = if matches!(edit, Edit::Undo) {
        idle(current)?;
        if current.deleted_at.is_some() { return Err("Restore this meeting from Trash first.".into()); }
        if current.edit_version != expected { return Err("This meeting changed. Reload it before undoing.".into()); }
        let version = current.undo_version.ok_or("There is no edit to undo.")?;
        let data = fs::read(folder.join("revisions").join(format!("{version}.json"))).map_err(|_| "The previous revision could not be read.")?;
        let mut previous: Meeting = serde_json::from_slice(&data).map_err(|_| "The previous revision is invalid.")?;
        if previous.id != current.id { return Err("The previous revision belongs to another meeting.".into()); }
        previous.edit_version = current.edit_version + 1;
        previous.revision = current.revision + 1;
        // Do not undo Trash implicitly or silently reinstate an obsolete sharing approval.
        previous.deleted_at = None;
        previous.notes_need_review = true;
        previous
    } else { edited(current, expected, edit)? };
    let revisions = folder.join("revisions");
    fs::create_dir_all(&revisions).map_err(|_| "Could not save edit history.")?;
    atomic_json(&revisions.join(format!("{}.json", current.edit_version)), current)?;
    atomic_json(&folder.join("meeting.json"), &next)?;
    Ok(next)
}

pub fn export_text(m: &Meeting, content: &str, format: &str) -> Result<String, String> {
    if m.deleted_at.is_some() { return Err("Restore this meeting before sharing it.".into()); }
    if !["notes", "transcript", "both"].contains(&content) || !["markdown", "text"].contains(&format) {
        return Err("Choose notes, transcript, or both, as Markdown or text.".into());
    }
    if content != "transcript" && m.notes_need_review { return Err("Refresh or review the notes after editing or redacting before sharing them.".into()); }
    let transcript = m.transcript();
    let text = match content {
        "notes" => m.notes.clone(),
        "transcript" => transcript,
        _ => format!("# {}\n\n{}\n\n## Transcript\n\n{}\n", m.title, m.notes, transcript),
    };
    if format == "markdown" { return Ok(text); }
    Ok(text.lines().map(|line| line.trim_start_matches('#').trim_start().replace("**", "").replace('`', "")).collect::<Vec<_>>().join("\n"))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hit { pub meeting_id: String, pub title: String, pub created_at: String, pub turn_id: String, pub start: f64, pub end: f64, pub text: String, pub speaker: String, pub score: usize }

pub fn search(meetings: &[Meeting], query: &str, limit: usize) -> Vec<Hit> {
    let words: Vec<_> = query.split_whitespace().map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()).filter(|w| !w.is_empty()).collect();
    let mut hits = Vec::new();
    for m in meetings.iter().filter(|m| m.deleted_at.is_none()) {
        for t in m.ordered_turns().into_iter().filter(|t| t.final_text) {
            // Search only material eligible for sharing, including word-level exclusions.
            let text = m.turn_transcript(t);
            if text.is_empty() { continue; }
            let haystack = format!("{} {}", m.title, text).to_lowercase();
            let score = words.iter().filter(|word| haystack.contains(word.as_str())).count();
            if !words.is_empty() && score == 0 { continue; }
            hits.push(Hit { meeting_id: m.id.clone(), title: m.title.clone(), created_at: m.created_at.clone(), turn_id: t.id.clone(), start: t.words.iter().find(|w|m.included(w.start,w.end)).map_or(t.start.max(m.trim_start),|w|w.start), end: t.words.iter().rev().find(|w|m.included(w.start,w.end)).map_or(t.end.min(m.trim_end.unwrap_or(m.duration)),|w|w.end), text, speaker: m.display_name(&t.speaker), score });
        }
    }
    hits.sort_by(|a,b| b.score.cmp(&a.score).then(a.meeting_id.cmp(&b.meeting_id)).then(a.start.total_cmp(&b.start)));
    hits.truncate(limit.min(200));
    hits
}

/// Write a trimmed mono WAV; redacted time ranges become silence in every channel.
pub fn render_audio(m: &Meeting, folder: &Path, channel: &str, output: &Path) -> Result<(), String> {
    idle(m)?;
    if m.deleted_at.is_some() { return Err("Restore this recording before playing it.".into()); }
    let names = match channel { "me" => vec!["microphone.wav"], "remote" => vec!["mac-audio.wav"], "both" => vec!["microphone.wav", "mac-audio.wav"], _ => return Err("Choose both, me, or remote audio.".into()) };
    let mut readers = Vec::new();
    for name in names {
        let reader = hound::WavReader::open(folder.join(name)).map_err(|_| "The recorded audio could not be opened.")?;
        let spec = reader.spec();
        if spec.channels != 1 || spec.bits_per_sample != 16 || spec.sample_format != hound::SampleFormat::Int { return Err("This recording has an unsupported audio format.".into()); }
        readers.push(reader);
    }
    let spec = readers[0].spec();
    if readers.iter().any(|r| r.spec() != spec) { return Err("The two audio sources have different formats.".into()); }
    let rate = spec.sample_rate as f64;
    let frames = readers.iter().map(|r| r.duration()).min().unwrap_or(0);
    let start = (m.trim_start * rate).ceil() as u32;
    let end = (m.trim_end.unwrap_or(m.duration) * rate).floor().min(frames as f64) as u32;
    if end <= start { return Err("No audio exists in the selected time range.".into()); }
    for reader in &mut readers { reader.seek(start).map_err(|_| "Could not seek the recorded audio.")?; }
    let parent = output.parent().ok_or("Invalid audio destination.")?;
    let temporary = tempfile::NamedTempFile::new_in(parent).map_err(|_| "Could not create the audio export.")?;
    {
        let mut writer = hound::WavWriter::new(temporary.as_file(), spec).map_err(|_| "Could not start the audio export.")?;
        let mut streams: Vec<_> = readers.iter_mut().map(|r| r.samples::<i16>()).collect();
        for frame in start..end {
            let mut mixed = 0i32;
            for stream in &mut streams { mixed += stream.next().ok_or("The recorded audio ended early.")?.map_err(|_| "Could not read the recorded audio.")? as i32; }
            let time = frame as f64 / rate;
            let hidden = m.redactions.iter().any(|r| time >= r.start && time < r.end);
            let value = if hidden { 0 } else { mixed.clamp(i16::MIN as i32, i16::MAX as i32) as i16 };
            writer.write_sample(value).map_err(|_| "Could not write the audio export. Check disk space.")?;
        }
        writer.finalize().map_err(|_| "Could not finish the audio export.")?;
    }
    temporary.persist(output).map_err(|_| "Could not save the audio export.")?;
    Ok(())
}

/// Reject references to sources that retrieval never supplied.
pub fn validate_citations(answer:&str,count:usize)->Result<(),String>{
    for chunk in answer.split('[').skip(1){
        if let Some(end)=chunk.find(']'){
            let label=&chunk[..end];
            if !label.is_empty() && label.bytes().all(|c|c.is_ascii_digit()){
                let index=label.parse::<usize>().map_err(|_|"The answer included an invalid citation. Ask again to retry.")?;
                if index==0 || index>count{return Err("The answer cited a source that was not found. Ask again to retry.".into());}
            }
        }
    }
    Ok(())
}

// New live turns do not invalidate an answer about unchanged cited passages.
pub fn sources_current(meetings:&[Meeting],sources:&[Hit])->bool{sources.iter().all(|s|meetings.iter().any(|m|m.id==s.meeting_id && m.deleted_at.is_none() && m.title==s.title && m.turns.get(&s.turn_id).is_some_and(|t|m.turn_transcript(t)==s.text && m.display_name(&t.speaker)==s.speaker)))}
