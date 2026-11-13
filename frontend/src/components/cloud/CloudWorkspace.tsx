'use client';
import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Mic, Monitor, Plus, Settings2, Square, Pause, Play, X, Check, Loader2, Copy, FolderOpen, RefreshCw, ArrowDown } from 'lucide-react';
import { Meeting, isActive, pending, speakerName, stamp, transcriptRows } from './types';
import './cloud.css';
type KeyStatus = { assemblyai: boolean; openai: boolean };
type History = { id: string; title: string; created_at: string; status: string };
const message = (e: unknown) => typeof e === 'string' ? e : 'Something went wrong. Please try again.';

function Connections({ close, changed }: { close: () => void; changed: (s: KeyStatus) => void }) {
  const [status, setStatus] = useState<KeyStatus>({ assemblyai: false, openai: false });
  const [assemblyai, setAssemblyai] = useState(''); const [openai, setOpenai] = useState('');
  const [busy, setBusy] = useState(false); const [error, setError] = useState(''); const [success, setSuccess] = useState('');
  useEffect(() => { invoke<KeyStatus>('cloud_key_status').then(setStatus).catch(e => setError(message(e))); }, []);
  async function save(test: boolean) {
    setBusy(true); setError(''); setSuccess('');
    try {
      const s = await invoke<KeyStatus>('cloud_save_keys', { assemblyai: assemblyai || null, openai: openai || null });
      setStatus(s); changed(s); setAssemblyai(''); setOpenai('');
      setSuccess(test ? await invoke<string>('cloud_test_keys') : 'Keys saved in Keychain.');
    } catch (e) { setError(message(e)); } finally { setBusy(false); }
  }
  return <div className="cl-modal-backdrop"><section className="cl-modal" role="dialog" aria-modal="true" aria-labelledby="connections-title">
    <div className="cl-heading"><h2 id="connections-title">Connections</h2><button className="cl-icon" aria-label="Close connections" onClick={close}><X size={20} /></button></div>
    <p className="cl-muted">Your API keys stay in macOS Keychain.</p>
    <form onSubmit={e => { e.preventDefault(); void save(true); }}>
      <label>AssemblyAI <span>{status.assemblyai ? 'Key saved' : 'Required'}</span><input autoFocus type="password" autoComplete="off" spellCheck={false} value={assemblyai} onChange={e => setAssemblyai(e.target.value)} placeholder={status.assemblyai ? 'Enter a key to replace it' : 'AssemblyAI API key'} /></label>
      <p className="cl-field-hint">Live transcription and speaker labels</p>
      <label>OpenAI <span>{status.openai ? 'Key saved' : 'Required'}</span><input type="password" autoComplete="off" spellCheck={false} value={openai} onChange={e => setOpenai(e.target.value)} placeholder={status.openai ? 'Enter a key to replace it' : 'OpenAI API key'} /></label>
      <p className="cl-field-hint">GPT-6.1 Terra · running notes and final summary</p>
      <p className="cl-privacy">Recording sends microphone and Mac audio to AssemblyAI, and transcript text to OpenAI. Audio and notes are also saved on this Mac. API usage is billed by each provider.</p>
      {error && <p className="cl-error" role="alert">{error}</p>}{success && <p className="cl-success" role="status"><Check size={16} /> {success}</p>}
      <div className="cl-actions"><button type="button" className="cl-button" disabled={busy} onClick={() => void save(false)}>Save keys</button><button type="submit" className="cl-button cl-primary" disabled={busy}>{busy ? <Loader2 className="cl-spin" size={16} /> : <Check size={16} />} Save & test</button></div>
    </form>
  </section></div>;
}
function Speaker({ meeting, id, rename }: { meeting: Meeting; id: string; rename: (id: string, name: string) => Promise<void> }) {
  const [editing, setEditing] = useState(false); const [name, setName] = useState('');
  if (id === 'me' || pending(id)) return <span className={`cl-speaker ${id === 'me' ? 'cl-me' : 'cl-them'}`}>{speakerName(meeting, id)}</span>;
  return editing ? <form className="cl-name-form" onSubmit={e => { e.preventDefault(); void rename(id, name).then(() => setEditing(false)).catch(() => {}); }}>
    <input aria-label={`Name for ${speakerName(meeting, id)}`} autoFocus list="participant-names" value={name} maxLength={80} onChange={e => setName(e.target.value)} placeholder="Confirm a name" />
    <button className="cl-icon" aria-label="Confirm speaker name"><Check size={15} /></button><button type="button" className="cl-icon" aria-label="Cancel naming" onClick={() => setEditing(false)}><X size={15} /></button>
  </form> : <button className="cl-speaker cl-them cl-name" title="Name this speaker" onClick={() => { setName(meeting.names[id] || ''); setEditing(true); }}>{speakerName(meeting, id)} <span>↗</span></button>;
}
function Meter({ label, value, remote = false }: { label: string; value: number; remote?: boolean }) {
  return <div className="cl-meter">{remote ? <Monitor size={14} /> : <Mic size={14} />}<span>{label}</span><div className="cl-meter-track"><i style={{ width: `${Math.min(100, Math.sqrt(value) * 230)}%` }} /></div></div>;
}
export default function CloudWorkspace() {
  const [meeting, setMeeting] = useState<Meeting | null>(null); const selected = useRef<string | null>(null);
  const [history, setHistory] = useState<History[]>([]); const [keys, setKeys] = useState<KeyStatus | null>(null);
  const [connections, setConnections] = useState(false); const [title, setTitle] = useState(''); const [participants, setParticipants] = useState('');
  const [microphone, setMicrophone] = useState(''); const [devices, setDevices] = useState<{ name: string; device_type: string }[]>([]);
  const [busy, setBusy] = useState(false); const [error, setError] = useState(''); const [notice, setNotice] = useState('');
  const [levels, setLevels] = useState({ me: 0, remote: 0, duration: 0 }); const [follow, setFollow] = useState(true);
  const transcript = useRef<HTMLDivElement>(null);
  const refreshHistory = useCallback(() => invoke<History[]>('cloud_list').then(setHistory).catch(e => setError(message(e))), []);
  useEffect(() => {
    let disposed = false; const off: UnlistenFn[] = [];
    async function initialize() {
      // Subscribe before loading: reloading the window cannot miss a completed turn.
      const a = await listen<Meeting>('cloud-meeting', ({ payload }) => {
        if (selected.current === payload.id || (!selected.current && isActive(payload))) { selected.current = payload.id; setMeeting(payload); }
        if (!isActive(payload)) void refreshHistory();
      }); if (disposed) { a(); return; } off.push(a);
      const b = await listen<{ id: string; me: number; remote: number; duration: number }>('cloud-levels', ({ payload }) => { if (selected.current === payload.id) setLevels(payload); }); if (disposed) { b(); return; } off.push(b);
      const [s, current] = await Promise.all([invoke<KeyStatus>('cloud_key_status'), invoke<Meeting | null>('cloud_current')]);
      if (disposed) return;
      setKeys(s); if (current) { selected.current = current.id; setMeeting(current); }
      setConnections(new URLSearchParams(window.location.search).has('connections') || !s.assemblyai || !s.openai);
      void refreshHistory();
      invoke<{ name: string; device_type: string }[]>('get_audio_devices').then(d => { if (!disposed) setDevices(d.filter(v => v.device_type === 'Input')); }).catch(() => {});
    }
    initialize().catch(e => { if (!disposed) setError(message(e)); });
    return () => { disposed = true; off.forEach(f => f()); };
  }, [refreshHistory]);
  const rows = meeting ? transcriptRows(meeting) : [];
  useEffect(() => { if (follow && transcript.current) transcript.current.scrollTop = transcript.current.scrollHeight; }, [meeting?.turns, follow]);
  async function action(work: () => Promise<unknown>) { setBusy(true); setError(''); try { await work(); } catch (e) { setError(message(e)); } finally { setBusy(false); } }
  async function start() {
    if (!keys?.assemblyai || !keys.openai) { setConnections(true); return; }
    await action(async () => {
      const m = await invoke<Meeting>('cloud_start', { title, participants: participants.split(/[,\n]/).map(s => s.trim()).filter(Boolean), microphone: microphone || null });
      selected.current = m.id; setMeeting(m); setLevels({ me: 0, remote: 0, duration: 0 }); setFollow(true); void refreshHistory();
      // Fetch the authoritative state in case native startup already published a newer event.
      setMeeting(await invoke<Meeting>('cloud_get', { id: m.id }));
    });
  }
  async function open(id: string) { await action(async () => { const m = await invoke<Meeting>('cloud_get', { id }); selected.current = id; setMeeting(m); setLevels({ me: 0, remote: 0, duration: m.duration }); setFollow(true); }); }
  async function rename(speaker: string, name: string) {
    try { await invoke('cloud_rename_speaker', { id: meeting!.id, speaker, name }); } catch (e) { setError(message(e)); throw e; }
  }
  const active = isActive(meeting); const recording = meeting?.status === 'recording' || meeting?.status === 'paused';
  const speakers = meeting ? Array.from(new Set(rows.map(r => r.speaker))).filter(id => id !== 'me' && !pending(id)) : [];
  return <div className="cl-app">
    <aside className="cl-sidebar"><img className="cl-logo" src="/logo.png" alt="PartyLine" /><button className="cl-button cl-primary cl-new" disabled={active || busy} onClick={() => { selected.current = null; setMeeting(null); setTitle(''); setParticipants(''); setError(''); }}><Plus size={17} /> New meeting</button>
      <div className="cl-history-label">Meetings</div><nav className="cl-history" aria-label="Saved meetings">{history.map(h => <button key={h.id} className={meeting?.id === h.id ? 'selected' : ''} disabled={active && meeting?.id !== h.id} onClick={() => void open(h.id)}><strong>{h.title}</strong><span>{new Date(h.created_at).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}{['recording', 'paused', 'connecting', 'finishing'].includes(h.status) ? ' · Live' : ''}</span></button>)}{!history.length && <p className="cl-muted">Your meetings will appear here.</p>}</nav>
      <button className="cl-button cl-connections" onClick={() => setConnections(true)}><Settings2 size={17} /> Connections <i className={keys?.assemblyai && keys.openai ? 'ready' : ''} /></button>
    </aside>
    <main className="cl-main">
      {error && <div className="cl-banner cl-error" role="alert">{error}<button className="cl-icon" aria-label="Dismiss error" onClick={() => setError('')}><X size={16} /></button></div>}
      {notice && <div className="cl-banner cl-success" role="status">{notice}<button className="cl-icon" aria-label="Dismiss notice" onClick={() => setNotice('')}><X size={16} /></button></div>}
      {!meeting ? <section className="cl-setup"><div className="cl-eyebrow">READY WHEN YOU ARE</div><h1>Keep the conversation moving.</h1><p className="cl-lead">Live transcript. Named speakers. Notes as you go.</p>
        <form onSubmit={e => { e.preventDefault(); void start(); }}><label>Meeting title<input placeholder="What’s the conversation?" value={title} maxLength={200} onChange={e => setTitle(e.target.value)} /></label>
          <label>Participants <span>Optional</span><textarea rows={2} placeholder="Paste names from your invite" value={participants} onChange={e => setParticipants(e.target.value)} /></label><p className="cl-field-hint">Confirm who’s speaking as voices appear. People can join at any time.</p>
          <label>Microphone<select value={microphone} onChange={e => setMicrophone(e.target.value)}><option value="">System default</option>{devices.map(d => <option key={d.name} value={d.name}>{d.name}</option>)}</select></label>
          <div className="cl-source-note"><Mic size={16} /> Your microphone → ME <Monitor size={16} /> Mac audio → speakers</div>
          <button className="cl-button cl-primary cl-start" disabled={busy || !keys} type="submit">{busy ? <Loader2 className="cl-spin" size={17} /> : <span className="cl-record-dot" />} Start recording</button>
          <p className="cl-field-hint">Use headphones to keep other voices out of your microphone. Let everyone know you’re recording.</p>
        </form>
      </section> : <>
        <header className="cl-toolbar"><div><div className="cl-status"><i className={meeting.status === 'recording' ? 'live' : ''} />{({ connecting: 'Connecting', recording: 'Recording', paused: 'Paused', finishing: 'Finishing notes', complete: 'Saved', interrupted: 'Interrupted' } as Record<string, string>)[meeting.status] || meeting.status}<span>{stamp(active ? Math.max(levels.duration, meeting.duration) : meeting.duration)}</span></div><h1>{meeting.title}</h1></div>
          <div className="cl-actions">{recording && <button className="cl-button" disabled={busy} onClick={() => void action(() => invoke('cloud_pause', { paused: meeting.status !== 'paused' }))}>{meeting.status === 'paused' ? <Play size={16} /> : <Pause size={16} />}{meeting.status === 'paused' ? 'Resume' : 'Pause'}</button>}{active && meeting.status !== 'finishing' && <button className="cl-button cl-stop" disabled={busy} onClick={() => void action(() => invoke('cloud_stop'))}><Square size={14} fill="currentColor" /> Stop</button>}{!active && <button className="cl-button" onClick={() => void action(() => invoke('cloud_reveal', { id: meeting.id }))}><FolderOpen size={16} /> Files</button>}</div>
        </header>
        {active && <div className="cl-sourcebar"><Meter label="ME" value={meeting.status === 'recording' ? levels.me : 0} /><Meter label="Mac audio" value={meeting.status === 'recording' ? levels.remote : 0} remote /><span className="cl-muted">{Object.values(meeting.connection).some(v => v !== 'Connected' && v !== 'Stopped') ? Object.entries(meeting.connection).map(([k,v]) => `${k === 'me' ? 'Mic' : 'Mac'}: ${v}`).join(' · ') : 'Audio saved on this Mac'}</span></div>}
        {meeting.errors.length > 0 && <details className="cl-warning"><summary>{meeting.errors[meeting.errors.length - 1]}</summary>{meeting.errors.slice(0,-1).map(e => <p key={e}>{e}</p>)}</details>}
        {speakers.length > 0 && <div className="cl-speakers"><span>Who’s speaking?</span>{speakers.map(id => <Speaker key={id} meeting={meeting} id={id} rename={rename} />)}</div>}
        <datalist id="participant-names">{meeting.participants.map((p, i) => <option key={i} value={p} />)}</datalist>
        <div className="cl-panels"><section className="cl-panel"><div className="cl-panel-heading"><h2>Transcript</h2><button className={`cl-text-button ${follow ? 'enabled' : ''}`} onClick={() => setFollow(v => !v)}><ArrowDown size={14} /> Follow</button></div><div className="cl-scroll cl-transcript" ref={transcript} onWheel={() => setFollow(false)}>
          {!rows.length && <div className="cl-empty"><Mic size={25} /><p>{meeting.status === 'connecting' ? 'Connecting your two audio sources…' : active ? 'Start talking. Words will appear here.' : 'No transcript was captured.'}</p></div>}
          {rows.map(row => <article key={row.id} className={`cl-turn ${!row.final ? 'cl-partial' : ''}`}><div><span className={`cl-speaker ${row.speaker === 'me' ? 'cl-me' : 'cl-them'}`}>{speakerName(meeting, row.speaker)}</span><time>{stamp(row.start)}</time>{!row.final && <span className="cl-muted">Live</span>}</div><p>{row.text}</p></article>)}
        </div></section>
        <section className="cl-panel cl-notes-panel"><div className="cl-panel-heading"><h2>Notes</h2><div className="cl-actions">{['updating','finalizing'].includes(meeting.notes_status) && <span className="cl-updating"><Loader2 size={13} className="cl-spin" />{meeting.notes_status === 'finalizing' ? 'Finalizing' : 'Updating'}</span>}<button className="cl-icon" title="Refresh notes" aria-label="Refresh notes" disabled={!rows.length || ['updating','finalizing'].includes(meeting.notes_status)} onClick={() => void action(() => invoke('cloud_refresh_notes', { id: meeting.id }))}><RefreshCw size={15} /></button><button className="cl-icon" title="Copy notes and transcript" aria-label="Copy notes and transcript" disabled={!rows.length} onClick={() => void action(async () => { const text = await invoke<string>('cloud_export', { id: meeting.id }); await navigator.clipboard.writeText(text); setNotice('Copied notes and transcript. A Markdown copy is saved in the meeting folder.'); })}><Copy size={15} /></button></div></div>
          <div className="cl-scroll">{meeting.notes_error && <p className="cl-error" role="alert">{meeting.notes_error}</p>}{!meeting.notes && !meeting.notes_draft ? <div className="cl-empty"><p>Decisions, next steps, and useful details will collect here.</p>{active && <span>Notes start after the first completed turn and refresh as the conversation continues.</span>}</div> : <div className="cl-markdown"><ReactMarkdown remarkPlugins={[remarkGfm]}>{meeting.notes_draft || meeting.notes}</ReactMarkdown></div>}{!active && meeting.notes_revision < meeting.revision && <p className="cl-field-hint">Speaker labels or text changed. Refresh notes to update them.</p>}</div>
        </section></div>
      </>}
    </main>{connections && <Connections close={() => setConnections(false)} changed={setKeys} />}
  </div>;
}
