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

#[test]
fn live_merges_apply_to_future_turns_preserve_names_and_undo() {
 let mut m=meeting();m.status="recording".into();
 for (i,label) in ["A","B","C"].iter().enumerate(){m.apply("remote",0,0.0,&serde_json::json!({"type":"Turn","turn_order":i,"transcript":"Hello","speaker_label":label,"end_of_turn":true,"words":[{"text":"Hello","start":0,"end":100,"speaker":label}]}));}
 m.names.insert("remote:0:A".into(),"Chad".into());
 m.merge_speakers(&["remote:0:B".into()],"remote:0:A",0).unwrap();
 assert_eq!(m.display_name("remote:0:B"),"Chad");
 m.apply("remote",0,1.0,&serde_json::json!({"type":"Turn","turn_order":5,"transcript":"Later","speaker_label":"B","end_of_turn":true}));
 assert!(m.transcript().contains("Chad: Later"));
 assert_eq!(m.turns["remote:0:5"].speaker,"remote:0:B");
 assert!(m.merge_speakers(&["remote:0:C".into()],"remote:0:A",0).is_err());
 m.merge_speakers(&["remote:0:A".into()],"me",1).unwrap();assert_eq!(m.display_name("remote:0:B"),"ME");
 m.undo_merge(2).unwrap();assert_eq!(m.display_name("remote:0:B"),"Chad");assert!(m.undo_merge(3).is_err());
 assert!(m.merge_speakers(&["me".into()],"remote:0:C",3).is_err());
 let roundtrip:Meeting=serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();assert_eq!(roundtrip.display_name("remote:0:B"),"Chad");
}
#[test]
fn recovery_changes_only_selected_channel_and_rejects_invalid_results() {
 let mut m=meeting();let original=m.clone();
 let response=serde_json::json!({"status":"completed","words":[{"text":"Recovered","start":1000,"end":1500,"speaker":"A"}]});
 lib::replace_recovered_channel(&mut m,"remote","test",&response).unwrap();
 assert_eq!(m.turns["me:0:0"].text,original.turns["me:0:0"].text);assert!(m.transcript().contains("Recovered speaker A: Recovered"));
 let before=m.transcript();assert!(lib::replace_recovered_channel(&mut m,"remote","test",&serde_json::json!({"status":"completed","words":[{"text":"bad","start":-1,"end":100}]})).is_err());assert_eq!(m.transcript(),before);
 assert!(lib::replace_recovered_channel(&mut m,"me","test",&serde_json::json!({"status":"completed","words":[]})).is_err());
 m.recovery.status="running".into();assert!(lib::edited(&m,m.edit_version,&Edit::Trash).is_err());
}
#[test]
fn preference_updates_validate_and_preserve_unrelated_settings() {
 use lib::model::WorkspacePreferences;
 let p=WorkspacePreferences::default();assert_eq!(p.notes_seconds,120);
 let p=p.patched(serde_json::json!({"notes_seconds":0,"sidebar_width":350,"sidebar_visible":false})).unwrap();assert_eq!(p.notes_seconds,0);assert!(!p.sidebar_visible);assert!(p.notes_visible);
 for patch in [serde_json::json!({"notes_seconds":1}),serde_json::json!({"transcript_percent":100}),serde_json::json!({"chat_width":100}),serde_json::json!({"api_key":"no"})] {assert!(p.patched(patch).is_err());}
 let copy:WorkspacePreferences=serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();assert_eq!(copy.sidebar_width,350);
}
#[test]
fn live_chat_citations_survive_new_turns_but_not_edits_redaction_or_trash() {
 let mut m=meeting();let hits=lib::search(&[m.clone()],"Public",10);assert!(lib::sources_current(&[m.clone()],&hits));
 m.apply("remote",0,3.0,&serde_json::json!({"type":"Turn","turn_order":0,"transcript":"New topic","speaker_label":"A","end_of_turn":true}));assert!(lib::sources_current(&[m.clone()],&hits));
 let redacted=lib::edited(&m,0,&Edit::Redact{start:1.0,end:2.0}).unwrap();assert!(!lib::sources_current(&[redacted],&hits));
 let trashed=lib::edited(&m,0,&Edit::Trash).unwrap();assert!(!lib::sources_current(&[trashed],&hits));
 let edited=lib::edited(&m,0,&Edit::Transcript{turn_id:"me:0:0".into(),text:"Changed".into(),start:None,end:None}).unwrap();assert!(!lib::sources_current(&[edited],&hits));
}

#[test]
fn outline_preferences_keep_defaults_for_new_nodes_and_round_trip() {
    use partyline_meeting_library::model::{OutlineState,OutlineChange};
    let original=OutlineState::default();
    let collapsed=original.changed(OutlineChange::CollapseAll).unwrap();
    assert!(collapsed.default_collapsed);
    let changed=collapsed.changed(OutlineChange::Set{node_id:"section/plan/0".into(),expanded:true}).unwrap();
    let folder=tempfile::tempdir().unwrap();
    partyline_meeting_library::atomic_json(&folder.path().join("outline-state.json"),&changed).unwrap();
    let loaded:OutlineState=serde_json::from_slice(&std::fs::read(folder.path().join("outline-state.json")).unwrap()).unwrap();
    assert_eq!(loaded.overrides.get("section/plan/0"),Some(&true));assert!(loaded.default_collapsed);
    let expanded=loaded.changed(OutlineChange::ExpandAll).unwrap();assert!(!expanded.default_collapsed);assert!(expanded.overrides.is_empty());
    assert!(original.changed(OutlineChange::Set{node_id:"".into(),expanded:false}).is_err());
    assert!(serde_json::from_value::<OutlineChange>(serde_json::json!({"operation":"set","node_id":"x","expanded":"false"})).is_err());
}

#[test]
fn legacy_collapsed_headings_keep_their_state_after_workspace_upgrade() {
    use lib::model::{OutlineState, OutlineChange};
    let state=OutlineState::from_legacy(&["root/Plan!/0".into(),"root/plan/0/Owners/0".into()]);
    assert_eq!(state.overrides["section/plan/0"],false);
    assert_eq!(state.overrides["section/owners/0"],false);
    let reset=state.changed(OutlineChange::ExpandAll).unwrap();
    assert!(reset.overrides.is_empty());
}

#[test]
fn live_details_keep_capture_and_streamed_content_and_cannot_undo_into_an_old_recording_snapshot() {
    let dir=tempfile::tempdir().unwrap();
    for status in ["connecting","recording","paused"] {
        let mut current=meeting();current.status=status.into();current.notes_status="updating".into();current.notes_draft="Notes in progress".into();
        let next=lib::save_edit(dir.path(),&current,0,&Edit::Details{title:"Workshop".into(),participants:vec!["Avery".into()]}).unwrap();
        assert_eq!(next.title,"Workshop");assert_eq!(next.participants,vec!["Avery"]);
        assert_eq!(next.status,status);assert_eq!(next.notes_status,"updating");assert_eq!(next.notes_draft,"Notes in progress");
        assert_eq!(next.transcript(),current.transcript());assert_eq!(next.duration,current.duration);
        assert_eq!(next.undo_version,None);assert_eq!(next.edit_version,1);
        let saved:Meeting=serde_json::from_slice(&fs::read(dir.path().join("meeting.json")).unwrap()).unwrap();assert_eq!(saved.title,"Workshop");
        let mut finished=next;finished.status="complete".into();finished.notes_status="idle".into();finished.duration=30.0;
        finished.apply("remote",0,20.0,&serde_json::json!({"type":"Turn","turn_order":1,"transcript":"Later speech must survive","speaker_label":"A","end_of_turn":true}));
        assert!(lib::save_edit(dir.path(),&finished,1,&Edit::Undo).is_err());
        let amended=lib::save_edit(dir.path(),&finished,1,&Edit::Details{title:"Workshop finished".into(),participants:vec![]}).unwrap();
        let undone=lib::save_edit(dir.path(),&amended,2,&Edit::Undo).unwrap();
        assert_eq!(undone.status,"complete");assert_eq!(undone.duration,30.0);assert_eq!(undone.transcript(),finished.transcript());
    }
}

#[test]
fn live_details_enforce_the_same_versions_validation_and_destructive_edit_guards() {
    let mut active=meeting();active.status="recording".into();
    let valid=Edit::Details{title:"New title".into(),participants:vec!["Avery".into()]};
    assert!(lib::edited(&active,1,&valid).is_err());
    assert!(lib::edited(&active,0,&Edit::Details{title:" ".into(),participants:vec![]}).is_err());
    assert!(lib::edited(&active,0,&Edit::Details{title:"Title".into(),participants:vec![" ".into()]}).is_err());
    for edit in [Edit::Trash,Edit::Trim{start:0.0,end:1.0},Edit::Redact{start:0.0,end:1.0},Edit::Notes{text:"Replace".into()}] {assert!(lib::edited(&active,0,&edit).is_err());}
    assert_eq!(active.title,"Original");assert_eq!(active.edit_version,0);
    active.status="finishing".into();assert!(lib::edited(&active,0,&valid).is_err());
}

#[test]
fn calendar_links_are_live_versioned_and_never_infer_speaker_identity() {
 let dir=tempfile::tempdir().unwrap();let mut m=meeting();m.status="recording".into();m.notes_status="updating".into();m.notes_draft="Streaming".into();
 let event=lib::calendar_model::event("account-a","primary",&serde_json::json!({"id":"event","summary":"Calendar title","start":{"dateTime":"2026-11-17T15:00:00-05:00"},"end":{"dateTime":"2026-11-17T16:00:00-05:00"},"attendees":[{"displayName":"Avery"}]})).unwrap();
 let linked=lib::save_edit(dir.path(),&m,0,&Edit::Calendar{event:Some(event.clone()),use_details:false}).unwrap();
 assert_eq!(linked.title,m.title);assert_eq!(linked.participants,m.participants);assert_eq!(linked.calendar_event,Some(event.clone()));assert_eq!(linked.undo_version,None);assert_eq!(linked.notes_draft,m.notes_draft);
 assert!(lib::edited(&linked,0,&Edit::Calendar{event:None,use_details:false}).is_err());
 let details=lib::save_edit(dir.path(),&linked,1,&Edit::Calendar{event:Some(event),use_details:true}).unwrap();assert_eq!(details.title,"Calendar title");assert_eq!(details.participants,vec!["Avery"]);assert_eq!(details.names,m.names);assert_eq!(details.turns.len(),m.turns.len());assert_eq!(details.status,"recording");
 let cleared=lib::save_edit(dir.path(),&details,2,&Edit::Calendar{event:None,use_details:false}).unwrap();assert!(cleared.calendar_event.is_none());assert_eq!(cleared.title,details.title);assert_eq!(cleared.participants,details.participants);
 let disk:Meeting=serde_json::from_slice(&fs::read(dir.path().join("meeting.json")).unwrap()).unwrap();assert_eq!(disk.edit_version,3);assert!(disk.calendar_event.is_none());
 let mut finishing=details;finishing.status="finishing".into();assert!(lib::edited(&finishing,2,&Edit::Calendar{event:None,use_details:false}).is_err());
}

#[test]
fn visible_citations_preserve_answer_order_and_original_source_numbers() {
 assert_eq!(lib::cited_source_indices("Answer [57] [79] [58], again [57]",80),vec![57,79,58]);
 assert_eq!(lib::cited_source_indices("[0] [81] [x] [] [9999999999999999999999999999999] [2]",80),vec![2]);
 assert!(lib::cited_source_indices("No citations",80).is_empty());
 assert!(lib::cited_source_indices("[1]",0).is_empty());
}
