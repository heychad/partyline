'use client';
import { useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Copy, Download, Pencil, Scissors, Trash2, Undo2, X } from 'lucide-react';
import { Meeting } from './types';
export type Edit = { kind: string; [key: string]: unknown };
export async function editMeeting(meeting: Meeting, edit: Edit) {
  return invoke<Meeting>('cloud_edit', { id: meeting.id, expectedVersion: meeting.edit_version ?? 0, edit });
}
export function EditDialog({ meeting, edit, close, saved }: { meeting: Meeting; edit: Edit; close: () => void; saved: (meeting: Meeting) => void }) {
  const baseline=useRef(meeting);
  const [draft,setDraft]=useState(edit); const [busy,setBusy]=useState(false); const [error,setError]=useState('');
  const patch=(key:string,value:unknown)=>setDraft(s=>({...s,[key]:value}));
  const label=({details:'Edit meeting',notes:'Edit notes',transcript:'Edit transcript turn',trim:'Trim recording',redact:'Redact time range',trash:'Move to Trash'} as Record<string,string>)[edit.kind] || 'Edit recording';
  async function save() {setBusy(true);setError('');try {const update=draft.kind==='details'?{...draft,participants:(draft.participants as string[]).map(p=>p.trim()).filter(Boolean)}:draft;saved(await editMeeting(baseline.current,update));close();}catch(e){setError(typeof e==='string'?e:'Could not save. Try again.');}finally{setBusy(false);}}
  return <div className="cl-modal-backdrop"><section className="cl-modal cl-editor" role="dialog" aria-modal="true" aria-label={label}><div className="cl-heading"><h2>{label}</h2><button className="cl-icon" aria-label="Close editor" disabled={busy} onClick={close}><X size={18}/></button></div>
    <form onSubmit={e=>{e.preventDefault();void save();}}>
      {edit.kind==='details' && <><label>Title<input autoFocus value={String(draft.title)} maxLength={200} onChange={e=>patch('title',e.target.value)}/></label><label>Participants<textarea value={(draft.participants as string[]).join('\n')} onChange={e=>patch('participants',e.target.value.split('\n'))}/></label></>}
      {['notes','transcript'].includes(edit.kind) && <><label>{edit.kind==='notes'?'Notes':'Transcript'}<textarea autoFocus rows={12} value={String(draft.text)} maxLength={edit.kind==='notes'?50000:16000} onChange={e=>patch('text',e.target.value)}/></label>{edit.kind==='notes' && meeting.notes_need_review && <p className="cl-field-hint">Remove any excluded material before saving. Saving marks these notes reviewed for sharing.</p>}</>}
      {['trim','redact'].includes(edit.kind) && <><div className="cl-range-fields"><label>Start (seconds)<input autoFocus type="number" min="0" max={meeting.duration} step="0.01" value={Number(draft.start)} onChange={e=>patch('start',Number(e.target.value))}/></label><label>End (seconds)<input type="number" min="0" max={meeting.duration} step="0.01" value={Number(draft.end)} onChange={e=>patch('end',Number(e.target.value))}/></label></div><p className="cl-field-hint">{edit.kind==='trim'?'Playback and exports use this range. Your original audio is kept.':'Shared transcript text is excluded and shared audio is silent during this range. Your original is kept.'}</p></>}
      {edit.kind==='trash' && <p className="cl-field-hint">The recording leaves your library. You can restore it from Trash. Existing copies elsewhere are unchanged.</p>}
      {error && <p className="cl-error" role="alert">{error}</p>}
      <div className="cl-actions"><button className="cl-button" type="button" disabled={busy} onClick={close}>Cancel</button><button className="cl-button cl-primary" disabled={busy} type="submit">{busy?'Saving…':edit.kind==='trash'?'Move to Trash':'Save'}</button></div>
    </form>
  </section></div>;
}
export function ShareDialog({ meeting, close }: { meeting: Meeting; close: () => void }) {
  const [content,setContent]=useState('notes');const [format,setFormat]=useState('markdown');const [busy,setBusy]=useState(false);const [status,setStatus]=useState('');const [error,setError]=useState('');
  async function share(clipboard:boolean,audio=false) {setBusy(true);setError('');setStatus('');try {if(audio){await invoke('cloud_audio',{id:meeting.id,channel:'both'});setStatus('Trimmed, redacted audio saved in the exports folder.');}else {await invoke('cloud_share',{id:meeting.id,content,format,clipboard});setStatus(clipboard?'Copied. Paste it into your destination.':'Saved in the exports folder.');}}catch(e){setError(typeof e==='string'?e:'Could not export. Try again.');}finally{setBusy(false);}}
  return <div className="cl-modal-backdrop"><section className="cl-modal" role="dialog" aria-modal="true" aria-label="Share recording"><div className="cl-heading"><h2>Share</h2><button className="cl-icon" aria-label="Close sharing" onClick={close}><X size={18}/></button></div><label>Content<select value={content} onChange={e=>setContent(e.target.value)}><option value="notes">Notes only</option><option value="transcript">Transcript only</option><option value="both">Notes and transcript</option></select></label><label>Format<select value={format} onChange={e=>setFormat(e.target.value)}><option value="markdown">Markdown</option><option value="text">Plain text</option></select></label>
    {meeting.notes_need_review && content!=='transcript' && <p className="cl-error">Refresh or review notes after your edits before sharing them.</p>}
    {error && <p className="cl-error" role="alert">{error}</p>}{status && <p className="cl-success" role="status">{status}</p>}
    <div className="cl-actions"><button className="cl-button" disabled={busy} onClick={()=>void share(false,true)}><Download size={16}/> Audio</button><button className="cl-button" disabled={busy} onClick={()=>void share(false)}><Download size={16}/> Export</button><button className="cl-button cl-primary" disabled={busy} onClick={()=>void share(true)}><Copy size={16}/> Copy</button></div>
    <button className="cl-text-button" onClick={()=>void invoke('cloud_reveal',{id:meeting.id}).catch(e=>setError(String(e)))}>Show saved files</button>
  </section></div>;
}
export function SavedActions({ meeting, edit, share, perform }: { meeting: Meeting; edit: (edit: Edit) => void; share: () => void; perform: (edit: Edit) => void }) {
  if(meeting.deleted_at) return <button className="cl-button cl-primary" onClick={()=>perform({kind:'restore'})}>Restore recording</button>;
  return <div className="cl-actions cl-saved-actions">
    <button className="cl-button" onClick={()=>edit({kind:'details',title:meeting.title,participants:meeting.participants})}><Pencil size={14}/> Edit</button>
    <button className="cl-button" onClick={()=>edit({kind:'trim',start:meeting.trim_start??0,end:meeting.trim_end??meeting.duration})}><Scissors size={14}/> Trim</button>
    <button className="cl-button" onClick={share}><Copy size={14}/> Share</button>
    <button className="cl-icon" aria-label="Undo last edit" title="Undo last edit" disabled={meeting.undo_version==null} onClick={()=>perform({kind:'undo'})}><Undo2 size={16}/></button>
    <button className="cl-icon" aria-label="Move recording to Trash" title="Move to Trash" onClick={()=>edit({kind:'trash'})}><Trash2 size={16}/></button>
  </div>;
}
