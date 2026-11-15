use partyline_meeting_library::{self as lib, model::{Meeting, Turn, Word}, Edit};
use std::{collections::BTreeMap, fs};
fn meeting() -> Meeting {
 let mut m=Meeting::new("Original".into(),vec!["Chad".into()]);
 m.status="complete".into(); m.duration=4.0; m.notes="Private phrase in earlier notes".into();
 m.turns=BTreeMap::from([("me:0:0".into(),Turn{id:"me:0:0".into(),channel:"me".into(),order:0,text:"Public secret after".into(),start:0.0,end:3.0,speaker:"me".into(),final_text:true,words:vec![Word{text:"Public".into(),start:0.0,end:1.0,speaker:"me".into()},Word{text:"secret".into(),start:1.0,end:2.0,speaker:"me".into()},Word{text:"after".into(),start:2.0,end:3.0,speaker:"me".into()}]})]);m
}
#[test]
fn edits_are_versioned_persisted_undoable_and_do_not_mutate_on_failure() {
 let dir=tempfile::tempdir().unwrap(); let original=meeting();
 let next=lib::save_edit(dir.path(),&original,0,&Edit::Details{title:"Renamed".into(),participants:vec!["Chad".into(),"Guest".into()]}).unwrap();
 assert_eq!(next.title,"Renamed"); assert_eq!(next.edit_version,1);
 assert!(lib::save_edit(dir.path(),&next,0,&Edit::Notes{text:"stale".into()}).is_err());
 let disk:Meeting=serde_json::from_slice(&fs::read(dir.path().join("meeting.json")).unwrap()).unwrap();assert_eq!(disk.title,"Renamed");
 let undo=lib::save_edit(dir.path(),&next,1,&Edit::Undo).unwrap();assert_eq!(undo.title,"Original");assert_eq!(undo.edit_version,2);assert_eq!(undo.undo_version,None);
 assert!(lib::save_edit(dir.path(),&undo,2,&Edit::Undo).is_err());
 assert_eq!(original.title,"Original");
}
#[test]
fn redactions_filter_search_and_text_and_block_old_notes() {
 let m=lib::edited(&meeting(),0,&Edit::Redact{start:1.0,end:2.0}).unwrap();
 let text=lib::export_text(&m,"transcript","markdown").unwrap();assert!(text.contains("Public"));assert!(!text.contains("secret"));
 assert!(lib::export_text(&m,"notes","markdown").is_err());assert!(lib::export_text(&m,"both","text").is_err());
 assert!(lib::search(&[m.clone()],"secret",50).is_empty());
 let reviewed=lib::edited(&m,1,&Edit::Notes{text:"Reviewed safe notes".into()}).unwrap();assert!(lib::export_text(&reviewed,"both","markdown").unwrap().contains("Reviewed safe notes"));
 let restored=lib::edited(&m,1,&Edit::Unredact{redaction_id:m.redactions[0].id.clone()}).unwrap();assert!(restored.transcript().contains("secret"));
}
#[test]
fn trim_excludes_partial_words_and_edit_discards_stale_word_alignment() {
 let m=lib::edited(&meeting(),0,&Edit::Trim{start:0.5,end:2.5}).unwrap();
 assert_eq!(m.transcript(),"[00:01] ME: secret");
 let changed=lib::edited(&meeting(),0,&Edit::Transcript{turn_id:"me:0:0".into(),text:"Corrected phrase".into(),start:None,end:None}).unwrap();
 assert!(changed.turns["me:0:0"].words.is_empty());assert_eq!(changed.transcript(),"[00:00] ME: Corrected phrase");
}
#[test]
fn trash_is_recoverable_and_cannot_be_shared_or_edited() {
 let mut original=meeting();original.notes_revision=original.revision;
 let m=lib::edited(&original,0,&Edit::Trash).unwrap();assert!(m.deleted_at.is_some());assert_eq!(m.notes_revision,m.revision);
 assert!(lib::search(&[m.clone()],"Public",20).is_empty());assert!(lib::export_text(&m,"transcript","text").is_err());
 assert!(lib::edited(&m,1,&Edit::Notes{text:"oops".into()}).is_err());
 let restored=lib::edited(&m,1,&Edit::Restore).unwrap();assert_eq!(restored.deleted_at,None);assert_eq!(restored.turns.len(),1);assert_eq!(restored.notes_revision,restored.revision);
 original.notes_revision=original.revision.saturating_sub(1);original.revision+=1;
 let stale=lib::edited(&original,0,&Edit::Trash).unwrap();assert!(stale.notes_revision<stale.revision);
}
#[test]
fn invalid_ranges_active_recordings_and_stale_edits_are_rejected() {
 for (start,end) in [(-1.0,1.0),(0.0,5.0),(2.0,1.0),(1.0,1.0),(f64::NAN,2.0)] {assert!(lib::edited(&meeting(),0,&Edit::Trim{start,end}).is_err());}
 let mut active=meeting();active.status="recording".into();assert!(lib::edited(&active,0,&Edit::Trash).is_err());
 let mut busy=meeting();busy.notes_status="updating".into();assert!(lib::edited(&busy,0,&Edit::Trash).is_err());
 assert!(lib::edited(&meeting(),99,&Edit::Trash).is_err());
}
#[test]
fn audio_trim_and_redaction_remove_samples_from_both_sources_and_preserve_originals() {
 let dir=tempfile::tempdir().unwrap();let spec=hound::WavSpec{channels:1,sample_rate:100,bits_per_sample:16,sample_format:hound::SampleFormat::Int};
 for name in ["microphone.wav","mac-audio.wav"] {let mut w=hound::WavWriter::create(dir.path().join(name),spec).unwrap();for _ in 0..400 {w.write_sample(1000i16).unwrap();}w.finalize().unwrap();}
 let original=fs::read(dir.path().join("microphone.wav")).unwrap();
 let trimmed=lib::edited(&meeting(),0,&Edit::Trim{start:0.5,end:3.5}).unwrap();let redacted=lib::edited(&trimmed,1,&Edit::Redact{start:1.0,end:2.0}).unwrap();
 let output=dir.path().join("shared.wav");lib::render_audio(&redacted,dir.path(),"both",&output).unwrap();
 let samples:Vec<i16>=hound::WavReader::open(&output).unwrap().samples().map(Result::unwrap).collect();
 assert_eq!(samples.len(),300);assert!(samples[..50].iter().all(|v| *v==2000));assert!(samples[50..150].iter().all(|v| *v==0));assert!(samples[150..].iter().all(|v| *v==2000));
 assert_eq!(fs::read(dir.path().join("microphone.wav")).unwrap(),original);
 lib::render_audio(&redacted,dir.path(),"me",&output).unwrap();assert_eq!(hound::WavReader::open(output).unwrap().samples::<i16>().next().unwrap().unwrap(),1000);
}
#[test]
fn failed_save_does_not_replace_a_recording() {
 let dir=tempfile::tempdir().unwrap();fs::write(dir.path().join("meeting.json"),b"original").unwrap();fs::write(dir.path().join("revisions"),b"blocked").unwrap();
 assert!(lib::save_edit(dir.path(),&meeting(),0,&Edit::Trash).is_err());assert_eq!(fs::read(dir.path().join("meeting.json")).unwrap(),b"original");
}

#[test]
fn editing_one_speaker_preserves_other_speakers_and_rejects_ambiguous_replacement() {
 let mut m=meeting();m.turns.get_mut("me:0:0").unwrap().words[2].speaker="remote:0:A".into();
 assert!(lib::edited(&m,0,&Edit::Transcript{turn_id:"me:0:0".into(),text:"Changed all".into(),start:None,end:None}).is_err());
 let updated=lib::edited(&m,0,&Edit::Transcript{turn_id:"me:0:0".into(),text:"Corrected first speaker".into(),start:Some(0.0),end:Some(2.0)}).unwrap();
 assert!(updated.transcript().contains("ME: Corrected first speaker"));assert!(updated.transcript().contains("Speaker A: after"));
 assert_eq!(updated.turns["me:0:0"].words[1].start,2.0);
}

#[test]
fn generated_citations_must_reference_real_retrieved_sources(){
 assert!(lib::validate_citations("Decision [1], action [2].",2).is_ok());
 assert!(lib::validate_citations("Invented [3].",2).is_err());
 assert!(lib::validate_citations("Invalid [0].",2).is_err());
 assert!(lib::validate_citations("No supporting evidence was found.",2).is_ok());
}

#[test]
fn retrieval_spans_calls_and_excludes_trashed_and_redacted_sources() {
 let first=meeting();let second=meeting();
 let trashed=lib::edited(&meeting(),0,&Edit::Trash).unwrap();
 let redacted=lib::edited(&meeting(),0,&Edit::Redact{start:0.0,end:3.0}).unwrap();
 let hits=lib::search(&[first.clone(),second.clone(),trashed,redacted],"secret",100);
 assert_eq!(hits.len(),2);
 assert!(hits.iter().any(|h|h.meeting_id==first.id));
 assert!(hits.iter().any(|h|h.meeting_id==second.id));
 assert!(hits.iter().all(|h|h.start==0.0 && h.end==3.0));
}
