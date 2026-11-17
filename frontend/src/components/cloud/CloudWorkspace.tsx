'use client';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import * as Popover from '@radix-ui/react-popover';
import { Mic, Monitor, Pencil, Settings2, Square, Pause, Play, X, Check, Loader2, Copy, FolderOpen, RefreshCw, ArrowDown, PanelLeft, PanelsTopLeft, MessageCircle, ChevronsDownUp, ChevronsUpDown, Users, ChevronDown, ShieldCheck } from 'lucide-react';
import { Meeting, isActive, pending, speakerName, stamp, transcriptRows } from './types';
import './cloud.css';
import NotesOutline from './NotesOutline';
import TranscriptPane from './TranscriptPane';
import NotesScroll from './NotesScroll';
import SetupCheck from './SetupCheck';
import QuickStart from './QuickStart';
import CalendarPanel, { CalendarPrompt, useCalendar } from './CalendarPanel';
import { transcriptGroups, transcriptPage } from './transcript';
import { emptyOutline, outlineDocument, type OutlineState } from './outline';
import ResizeDivider from './ResizeDivider';
import { defaults, Preferences, refreshRates } from './workspace';
import LibrarySettings from './LibrarySettings';
import LibraryChat from './LibraryChat';
import PlaybackBar from './PlaybackBar';
import { PlaybackController, PlaybackRequest, emptyPlayback } from './playback';
import { Edit, EditDialog, ShareDialog, SavedActions, editMeeting } from './SavedMeetingTools';
import AgentAccess from '../agent-control/AgentAccess';
import { useCloudAgentBridge } from '../agent-control/useCloudAgentBridge';
type KeyStatus = { assemblyai: boolean; openai: boolean };
type History = { id: string; title: string; created_at: string; status: string };
const message = (e: unknown) => typeof e === 'string' ? e : 'Something went wrong. Please try again.';

function Connections({ close, changed }: { close: () => void; changed: (s: KeyStatus) => void }) {
  const [status, setStatus] = useState<KeyStatus>({ assemblyai: false, openai: false });
  const [assemblyai, setAssemblyai] = useState(''); const [openai, setOpenai] = useState('');
  const [busy, setBusy] = useState(false); const [error, setError] = useState(''); const [success, setSuccess] = useState('');
  useEffect(() => { invoke<KeyStatus>('cloud_key_status').then(setStatus).catch(e => setError(message(e))); }, []);
  useEffect(() => {
    let disposed = false; let off: UnlistenFn | undefined;
    void listen<KeyStatus>('partyline-agent-connections', ({ payload }) => setStatus(payload))
      .then(f => { if (disposed) f(); else off = f; });
    return () => { disposed = true; off?.(); };
  }, []);
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
    <SetupCheck/>
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
  const calendar=useCalendar();const [calendarOpen,setCalendarOpen]=useState(false);
  const [meeting, setMeeting] = useState<Meeting | null>(null); const selected = useRef<string | null>(null);
  const [history, setHistory] = useState<History[]>([]); const [keys, setKeys] = useState<KeyStatus | null>(null);
  const [connections, setConnections] = useState(false); const [title, setTitle] = useState(''); const [participants, setParticipants] = useState('');
  const [microphone, setMicrophone] = useState(''); const [devices, setDevices] = useState<{ name: string; device_type: string }[]>([]);
  const [busy, setBusy] = useState(false); const [error, setError] = useState(''); const [notice, setNotice] = useState('');
  const [levels, setLevels] = useState({ me: 0, remote: 0, duration: 0 }); const [follow, setFollow] = useState(true);
  const [viewOpen,setViewOpen]=useState(false); const [mergeOpen,setMergeOpen]=useState(false);
  const openPopover=useRef<string|null>(null);openPopover.current=mergeOpen?'speakers':viewOpen?'view':null;
  const [outlineState,setOutlineState]=useState<OutlineState>(emptyOutline);
  const [outlineReady,setOutlineReady]=useState(false);
  const noteText=meeting?.notes || meeting?.notes_draft || '';
  const noteDocument=useMemo(()=>outlineDocument(noteText,outlineState),[noteText,outlineState]);
  useEffect(()=>{
    let disposed=false;let off:UnlistenFn|undefined;setOutlineReady(false);setOutlineState(emptyOutline);
    if(!meeting)return;
    const id=meeting.id;
    void (async()=>{
      off=await listen<{id:string;state:OutlineState}>('cloud-outline',({payload})=>{if(!disposed&&payload.id===id)setOutlineState(payload.state);});
      if(disposed){off();return;}
      const state=await invoke<OutlineState>('cloud_outline',{id});
      if(!disposed){setOutlineState(state);setOutlineReady(true);}
    })().catch(e=>{if(!disposed)setError(message(e));});
    return()=>{disposed=true;off?.();};
  },[meeting?.id]);
  function validateOutline(id:unknown,change:Record<string,unknown>){
    if(!meeting||id!==meeting.id||!outlineReady)throw new Error('Open this recording and wait for its outline to load.');
    if(change.operation==='set'&&!noteDocument.nodes.some(n=>n.id===change.node_id))throw new Error('Outline section changed. Read the outline and retry.');
  }
  async function changeOutline(change:Record<string,unknown>){
    const id=meeting?.id;validateOutline(id,change);
    const state=await invoke<OutlineState>('cloud_outline_update',{id,change});
    if(selected.current===id)setOutlineState(state);return state;
  }
  const outlineAction=(change:Record<string,unknown>)=>void changeOutline(change).catch(e=>setError(message(e)));
  const [query,setQuery]=useState(''); const [trash,setTrash]=useState(false);
  const [settingsOpen,setSettingsOpen]=useState(false);
  const [prefs,setPrefs]=useState<Preferences>(defaults);const chatOpen=prefs.chat_visible;
  const [mergeSources,setMergeSources]=useState<string[]>([]);const [mergeTarget,setMergeTarget]=useState('');const [recoveryChannel,setRecoveryChannel]=useState('both');
  async function configure(patch:Partial<Preferences>){try{setPrefs(await invoke<Preferences>('cloud_preferences_update',{patch}));}catch(e){setError(message(e));}}
  const setChatOpen=(open:boolean)=>{void configure({chat_visible:open});};
  useEffect(()=>{let disposed=false;let off:UnlistenFn|undefined;void listen<Preferences>('cloud-preferences',({payload})=>setPrefs(payload)).then(f=>{if(disposed)f();else off=f;});void invoke<Preferences>('cloud_preferences').then(p=>{if(!disposed)setPrefs(p);}).catch(e=>setError(message(e)));return()=>{disposed=true;off?.();};},[]);
  const [editor,setEditor]=useState<Edit | null>(null); const [sharing,setSharing]=useState(false);
  const [playback,setPlayback]=useState(emptyPlayback); const player=useRef<PlaybackController | null>(null);
  useEffect(()=>{const audio=new Audio();player.current=new PlaybackController(audio,(id,channel)=>invoke('cloud_audio',{id,channel}),convertFileSrc,setPlayback);return()=>{player.current?.close();player.current=null;};},[]);
  useEffect(()=>{player.current?.observeRecording(meeting?.id??null,meeting?.edit_version??0);},[meeting?.id,meeting?.edit_version]);
  async function play(request: PlaybackRequest) { if(selected.current!==request.id) await open(request.id); if(!player.current) throw new Error('Playback is unavailable.'); await player.current.set(request); }
  async function performEdit(edit: Edit) { if(meeting) await action(async()=>{const m=await editMeeting(meeting,edit);setMeeting(m);void refreshHistory();}); }

  const historyRequest=useRef(0);
  const refreshHistory = useCallback(() => {const request=++historyRequest.current;return invoke<{items:History[]}>('cloud_library',{trash,query}).then(r=>{if(request===historyRequest.current)setHistory(r.items);}).catch(e => {if(request===historyRequest.current)setError(message(e));});}, [trash,query]);
  useEffect(()=>{void refreshHistory();},[refreshHistory]);
  const refreshRef=useRef(refreshHistory); refreshRef.current=refreshHistory;
  useEffect(() => {
    let disposed = false; const off: UnlistenFn[] = [];
    async function initialize() {
      // Subscribe before loading: reloading the window cannot miss a completed turn.
      const a = await listen<Meeting>('cloud-meeting', ({ payload }) => {
        if (selected.current === payload.id || (!selected.current && isActive(payload))) { selected.current = payload.id; setMeeting(payload); }
        if (!isActive(payload)) void refreshRef.current();
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
  }, []);
  const rows = useMemo(()=>meeting ? transcriptRows(meeting) : [],[meeting?.turns,meeting?.speaker_aliases]);
  const groups = useMemo(()=>transcriptGroups(rows),[rows]);
  async function action(work: () => Promise<unknown>) { setBusy(true); setError(''); try { await work(); } catch (e) { setError(message(e)); } finally { setBusy(false); } }
  async function start(quick = false) {

    player.current?.close();
    await action(async () => {
      const m = await invoke<Meeting>('cloud_start', { title: quick ? '' : title, participants: quick ? [] : participants.split(/[,\n]/).map(s => s.trim()).filter(Boolean), microphone: microphone || null });
      selected.current = m.id; setMeeting(m); setLevels({ me: 0, remote: 0, duration: 0 }); setFollow(true); void refreshHistory();
      // Fetch the authoritative state in case native startup already published a newer event.
      setMeeting(await invoke<Meeting>('cloud_get', { id: m.id }));
    });
  }
  async function open(id: string) { setEditor(null);setSharing(false);if(selected.current!==id)player.current?.close(); await action(async () => { const m = await invoke<Meeting>('cloud_get', { id }); selected.current = id; setMeeting(m); setLevels({ me: 0, remote: 0, duration: m.duration }); setFollow(isActive(m)); }); }
  async function rename(speaker: string, name: string) {
    try { await invoke('cloud_rename_speaker', { id: meeting!.id, speaker, name }); } catch (e) { setError(message(e)); throw e; }
  }
  const active = isActive(meeting); const recording = meeting?.status === 'recording' || meeting?.status === 'paused';
  const speakers = meeting ? Array.from(new Set(rows.map(r => r.speaker))).filter(id => id !== 'me' && !pending(id)) : [];
  useCloudAgentBridge({
    view: { selected_id: meeting?.id ?? null, follow, connections_open: connections,
      draft: { title, participants: participants.split(/[,\n]/).map(s => s.trim()).filter(Boolean), microphone }, playback, workspace:{query,trash,preferences:prefs,panel:calendarOpen?'calendar':settingsOpen?'settings':viewOpen?'view':mergeOpen?'speakers':chatOpen?'chat':sharing?'share':editor?.kind??'none',editor,outline:{meeting_id:meeting?.id??null,ready:outlineReady,state:outlineState,nodes:noteDocument.nodes},transcript:{groups:groups.length}} },
    busy,
    open: async id => { await open(id); if (selected.current !== id) throw new Error('Meeting did not open.'); },
    newMeeting: () => { player.current?.close();setEditor(null);setSharing(false); selected.current = null; setMeeting(null); setTitle(''); setParticipants(''); setError(''); },
    prepare: draft => {
      player.current?.close();setEditor(null);setSharing(false); selected.current = null; setMeeting(null); setError('');
      if (draft.title !== undefined) setTitle(draft.title);
      if (draft.participants !== undefined) setParticipants(draft.participants.join(', '));
      if (draft.microphone !== undefined) setMicrophone(draft.microphone);
    },
    setFollow, setConnections,
    dismiss: target => target === 'error' ? setError('') : setNotice(''),
    copied: () => setNotice('Copied notes and transcript. A Markdown copy is saved in the meeting folder.'),
    keysChanged: setKeys, playback: play, closePlayback: ()=>player.current?.close(),
    workspace: async request=>{
      if(['outline','transcript','outline_validate'].includes(String(request.action))){
        if(!meeting||request.id!==meeting.id)throw new Error('Open the requested recording first.');
        if(request.action==='transcript')return {meeting_id:meeting.id,...transcriptPage(groups,Number(request.offset??0),Number(request.limit??50))};
        if(request.action==='outline_validate'){validateOutline(request.id,request.change as Record<string,unknown>);return {ok:true};}
        return {meeting_id:meeting.id,ready:outlineReady,state:outlineState,nodes:noteDocument.nodes};
      }
      const panel=request.panel as string | undefined;
      if(panel && !['none','calendar','settings','chat','view','speakers'].includes(panel) && (!meeting || (active && panel!=='details') || meeting.deleted_at)) throw new Error('Open a saved recording first.');
      if(panel==='transcript' && !meeting?.turns[String(request.turn_id)]) throw new Error('Transcript turn not found.');
      if(request.query!==undefined)setQuery(String(request.query));if(request.trash!==undefined)setTrash(request.trash===true);
      if(panel==='speakers'&&!meeting)throw new Error('Open a recording first.');
      if(panel!==undefined){setCalendarOpen(panel==='calendar');setPrefs(await invoke<Preferences>('cloud_preferences_update',{patch:{chat_visible:panel==='chat'}}));setViewOpen(panel==='view');setMergeOpen(panel==='speakers');setSettingsOpen(panel==='settings');setSharing(panel==='share');setEditor(null);
        if(meeting){if(panel==='details')setEditor({kind:'details',title:meeting.title,participants:meeting.participants});if(panel==='notes')setEditor({kind:'notes',text:meeting.notes});if(['trim','redact'].includes(panel))setEditor({kind:panel,start:meeting.trim_start??0,end:meeting.trim_end??meeting.duration});if(panel==='transcript'){const turn=meeting.turns[String(request.turn_id)];if(!turn)throw new Error('Transcript turn not found.');setEditor({kind:'transcript',turn_id:turn.id,text:turn.text});}}
      }
    },
  });
  return <div className="cl-app">
    {prefs.sidebar_visible && <><aside className="cl-sidebar" style={{width:prefs.sidebar_width}}><img className="cl-logo" src="/logo.png" alt="PartyLine" /><button className="cl-button cl-primary cl-new" disabled={active || busy} onClick={() => void start(true)}><Mic size={17} /> Start recording</button>
      <div className="cl-library-filter"><input aria-label="Search recordings" placeholder="Search recordings" value={query} onChange={e=>setQuery(e.target.value)}/><button className="cl-text-button" aria-pressed={trash} onClick={()=>setTrash(v=>!v)}>{trash?"Back to recordings":"Trash"}</button></div><nav className="cl-history" aria-label="Saved meetings">{history.map(h => <button key={h.id} className={meeting?.id === h.id ? 'selected' : ''} disabled={active && meeting?.id !== h.id} onClick={() => void open(h.id)}><strong>{h.title}</strong><span>{new Date(h.created_at).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}{['recording', 'paused', 'connecting', 'finishing'].includes(h.status) ? ' · Live' : ''}</span></button>)}{!history.length && <p className="cl-muted">{query ? 'No matching recordings.' : trash ? 'Trash is empty.' : 'Your meetings will appear here.'}</p>}</nav>
      <div className="cl-sidebar-bottom"><button className="cl-button" onClick={()=>setChatOpen(true)}>Ask recordings</button><button className="cl-text-button" onClick={()=>setSettingsOpen(true)}>Settings</button></div>
      <AgentAccess />
      <button className="cl-button cl-connections" onClick={() => setConnections(true)}><Settings2 size={17} /> Connections <i className={keys?.assemblyai && keys.openai ? 'ready' : ''} /></button>
    </aside><ResizeDivider label="Library width" value={prefs.sidebar_width} min={150} max={400} preview={sidebar_width=>setPrefs(p=>({...p,sidebar_width}))} change={sidebar_width=>void configure({sidebar_width})}/></>}
    <main className={`cl-main ${prefs.pin_transcript?'':'cl-unpinned'}`}>
      <div className="cl-workspace-bar"><button className="cl-icon" aria-label="Toggle library" aria-pressed={prefs.sidebar_visible} onClick={()=>void configure({sidebar_visible:!prefs.sidebar_visible})}><PanelLeft size={17}/></button><span className="cl-breadcrumb">Recordings{meeting?' / Meeting':''}</span><button className="cl-button cl-quiet" aria-pressed={calendarOpen} onClick={()=>setCalendarOpen(v=>!v)}>Calendar</button>
        <Popover.Root open={viewOpen} onOpenChange={setViewOpen}><Popover.Trigger asChild><button className="cl-button cl-quiet"><PanelsTopLeft size={16}/>View<ChevronDown size={12}/></button></Popover.Trigger><Popover.Content className="cl-popover" align="end" sideOffset={8} aria-label="Workspace layout" onCloseAutoFocus={e=>{if(openPopover.current==='speakers')e.preventDefault();}}>
          <h3>Workspace</h3>{([['sidebar_visible','Library'],['transcript_visible','Transcript'],['notes_visible','Notes']] as const).map(([key,label])=><label className="cl-check" key={key}><input type="checkbox" checked={prefs[key]} onChange={()=>void configure({[key]:!prefs[key]})}/>{label}</label>)}
          <label className="cl-check"><input type="checkbox" checked={prefs.pin_transcript} onChange={()=>void configure({pin_transcript:!prefs.pin_transcript})}/>Scroll panels independently</label>
          <button className="cl-text-button" onClick={()=>{void configure({sidebar_width:defaults.sidebar_width,transcript_percent:defaults.transcript_percent,chat_width:defaults.chat_width,sidebar_visible:true,transcript_visible:true,notes_visible:true,pin_transcript:true});setViewOpen(false);}}>Reset layout</button>
        </Popover.Content></Popover.Root>
        <button className={`cl-button cl-quiet ${chatOpen?'enabled':''}`} aria-pressed={chatOpen} onClick={()=>setChatOpen(!chatOpen)}><MessageCircle size={16}/>Ask</button>
      </div>
      {error && <div className="cl-banner cl-error" role="alert">{error}<button className="cl-icon" aria-label="Dismiss error" onClick={() => setError('')}><X size={16} /></button></div>}
      {notice && <div className="cl-banner cl-success" role="status">{notice}<button className="cl-icon" aria-label="Dismiss notice" onClick={() => setNotice('')}><X size={16} /></button></div>}
      <CalendarPrompt state={calendar.state} active={active} start={()=>void start(true)} open={()=>setCalendarOpen(true)} busy={busy}/>
      {!meeting ? <QuickStart title={title} participants={participants.split(/[,\n]/).map(s=>s.trim()).filter(Boolean)} microphone={microphone} devices={devices} busy={busy || !keys} start={()=>void start()} selectMicrophone={setMicrophone} connections={()=>setConnections(true)}/> : <>
        <header className="cl-toolbar"><div><div className="cl-status"><i className={meeting.status === 'recording' ? 'live' : ''} />{({ connecting: 'Connecting', recording: 'Recording', paused: 'Paused', finishing: 'Finishing notes', complete: 'Saved', interrupted: 'Interrupted' } as Record<string, string>)[meeting.status] || meeting.status}<span>{stamp(active ? Math.max(levels.duration, meeting.duration) : meeting.duration)}</span></div><h1><button className="cl-title-edit" disabled={!!meeting.deleted_at} aria-label="Edit meeting details" onClick={()=>setEditor({kind:'details',title:meeting.title,participants:meeting.participants})}>{meeting.title}<Pencil size={15}/></button></h1></div>
          <div className="cl-actions">{!active && <SavedActions meeting={meeting} edit={setEditor} share={()=>setSharing(true)} perform={edit=>void performEdit(edit)}/>}</div>
        </header>
        {!active && !meeting.deleted_at && <div className="cl-library-actions"><button className="cl-text-button" onClick={()=>void action(async()=>{await invoke('cloud_backup_start',{id:meeting.id});setNotice('Backup started. Track progress in Settings.');})}>Back up to Drive</button><button className="cl-button" onClick={()=>void play({id:meeting.id,position:meeting.trim_start??0,playing:true}).catch(e=>setError(message(e)))}><Play size={15}/> Listen</button><span className="cl-muted">{meeting.trim_start || meeting.trim_end ? `Selected range ${stamp(meeting.trim_start??0)}–${stamp(meeting.trim_end??meeting.duration)}` : 'Click a transcript passage to listen'}</span><button className="cl-text-button" onClick={()=>setEditor({kind:'redact',start:meeting.trim_start??0,end:meeting.trim_end??meeting.duration})}>Redact range</button>{meeting.redactions?.map(r=><button className="cl-text-button" key={r.id} onClick={()=>void performEdit({kind:'unredact',redaction_id:r.id})}>Unredact {stamp(r.start)}–{stamp(r.end)}</button>)}</div>}

        {!active && !meeting.deleted_at && <details className="cl-recovery"><summary>Recover transcript from saved audio{meeting.recovery_needed?' · Recommended':''}</summary><p className="cl-field-hint">Reprocess the selected original audio with AssemblyAI, including trimmed or redacted passages. Its transcript will be replaced; originals and undo are kept. Confirm recovered speakers and refresh notes afterward. Provider charges apply.</p><div className="cl-actions"><select aria-label="Recovery audio channel" value={recoveryChannel} onChange={e=>setRecoveryChannel(e.target.value)}><option value="both">Both channels</option><option value="me">Microphone</option><option value="remote">Mac audio</option></select><button className="cl-button" disabled={busy||meeting.recovery?.status==='running'} onClick={()=>void action(()=>invoke('cloud_recovery_start',{id:meeting.id,channel:recoveryChannel,expectedVersion:meeting.edit_version??0}))}>Recover transcript</button>{meeting.recovery?.status==='running'&&<button className="cl-text-button" onClick={()=>void action(()=>invoke('cloud_recovery_cancel',{id:meeting.id}))}>Cancel recovery</button>}</div>{meeting.recovery?.status&&<p role="status">{meeting.recovery.status} · {meeting.recovery.phase} · {meeting.recovery.progress}%</p>}{meeting.recovery?.error&&<p className="cl-error" role="alert">{meeting.recovery.error}</p>}</details>}
        {meeting.errors.length > 0 && <details className="cl-warning"><summary>{meeting.errors[meeting.errors.length - 1]}</summary>{meeting.errors.slice(0,-1).map(e => <p key={e}>{e}</p>)}</details>}
        {speakers.length > 0 && <div className="cl-speakers"><span>Speakers</span><div className="cl-speaker-chips">{speakers.map(id => <Speaker key={id} meeting={meeting} id={id} rename={rename} />)}</div>
          <Popover.Root open={mergeOpen} onOpenChange={setMergeOpen}><Popover.Trigger asChild><button className="cl-icon" aria-label="Manage speakers"><Users size={16}/></button></Popover.Trigger><Popover.Content className="cl-popover cl-speaker-popover" align="end" sideOffset={8} aria-label="Merge speakers" onCloseAutoFocus={e=>{if(openPopover.current==='view')e.preventDefault();}}><h3>Merge speakers</h3><p className="cl-field-hint">Choose entries that belong to the same person.</p><div className="cl-merge-options">{speakers.map(id=><label className="cl-check" key={id}><input type="checkbox" checked={mergeSources.includes(id)} onChange={e=>{setMergeSources(old=>e.target.checked?[...old,id]:old.filter(s=>s!==id));if(mergeTarget===id)setMergeTarget('');}}/>{speakerName(meeting,id)}</label>)}</div><label>Keep this identity<select aria-label="Merge destination" value={mergeTarget} onChange={e=>setMergeTarget(e.target.value)}><option value="">Choose a speaker</option>{['me',...speakers].filter(id=>!mergeSources.includes(id)).map(id=><option key={id} value={id}>{speakerName(meeting,id)}</option>)}</select></label><button className="cl-button cl-primary" disabled={!mergeSources.length||!mergeTarget||mergeSources.includes(mergeTarget)||busy} onClick={()=>void action(async()=>{await invoke('cloud_merge_speakers',{id:meeting.id,sources:mergeSources,target:mergeTarget,expectedVersion:meeting.speaker_edit_version??0});setMergeSources([]);setMergeTarget('');setMergeOpen(false);})}>Merge speakers</button></Popover.Content></Popover.Root>
          {meeting.previous_aliases&&<button className="cl-text-button" onClick={()=>void action(()=>invoke('cloud_undo_speaker_merge',{id:meeting.id,expectedVersion:meeting.speaker_edit_version??0}))}>Undo merge</button>}
        </div>}
        <datalist id="participant-names">{meeting.participants.map((p, i) => <option key={i} value={p} />)}</datalist>
        <div className="cl-panels" style={{gridTemplateColumns:prefs.transcript_visible&&prefs.notes_visible?`minmax(0,${prefs.transcript_percent}fr) 7px minmax(0,${100-prefs.transcript_percent}fr)`:'1fr'}}>{prefs.transcript_visible && <TranscriptPane meeting={meeting} groups={groups} active={active} independent={prefs.pin_transcript} follow={follow} setFollow={setFollow} play={position=>void play({id:meeting.id,position,playing:true}).catch(e=>setError(message(e)))} edit={row=>setEditor({kind:'transcript',turn_id:row.turn_id,text:meeting.turns[row.turn_id].text})} redact={row=>setEditor({kind:'redact',start:row.start,end:Math.min(meeting.duration,row.end)})}/>}
        {prefs.transcript_visible&&prefs.notes_visible&&<ResizeDivider label="Transcript column width" percent value={prefs.transcript_percent} min={20} max={80} preview={transcript_percent=>setPrefs(p=>({...p,transcript_percent}))} change={transcript_percent=>void configure({transcript_percent})}/>}
        {prefs.notes_visible && <section className="cl-panel cl-notes-panel"><div className="cl-panel-heading"><h2>Notes</h2><div className="cl-actions"><button className="cl-icon" aria-label="Collapse all notes" title="Collapse all" disabled={!outlineReady||!noteDocument.nodes.length} onClick={()=>outlineAction({operation:'collapse_all'})}><ChevronsDownUp size={15}/></button><button className="cl-icon" aria-label="Expand all notes" title="Expand all" disabled={!outlineReady||!noteDocument.nodes.length} onClick={()=>outlineAction({operation:'expand_all'})}><ChevronsUpDown size={15}/></button>{!active && !meeting.deleted_at && <button className="cl-text-button" onClick={()=>setEditor({kind:"notes",text:meeting.notes})}>Edit notes</button>}{['updating','finalizing'].includes(meeting.notes_status) && <span className="cl-updating"><Loader2 size={13} className="cl-spin" />{meeting.notes_status === 'finalizing' ? 'Finalizing' : 'Updating'}</span>}<button className="cl-icon" title="Refresh notes" aria-label="Refresh notes" disabled={!rows.length || ['updating','finalizing'].includes(meeting.notes_status)} onClick={() => void action(() => invoke('cloud_refresh_notes', { id: meeting.id }))}><RefreshCw size={15} /></button><button className="cl-icon" title="Copy notes" aria-label="Copy notes" disabled={!rows.length} onClick={() => void action(async () => { await invoke('cloud_share', { id: meeting.id, content: 'notes', format: 'markdown', clipboard: true }); setNotice('Copied notes.'); })}><Copy size={15} /></button></div></div>
          <div className="cl-notes-meta"><span>{['updating','finalizing'].includes(meeting.notes_status)?'Updating notes…':'Running notes'}</span><Popover.Root><Popover.Trigger className="cl-text-button" aria-label="Change notes refresh interval">{prefs.notes_seconds===0?'Manual':prefs.notes_seconds<60?'Every 30 sec':`Every ${prefs.notes_seconds/60} min`}<ChevronDown size={12}/></Popover.Trigger><Popover.Content className="cl-popover" align="end" sideOffset={6}><label className="cl-refresh">Refresh notes: {prefs.notes_seconds===0?'Manual':`${prefs.notes_seconds} seconds`}<input type="range" aria-label="Notes refresh interval" min={0} max={4} step={1} value={refreshRates.indexOf(prefs.notes_seconds)} onChange={e=>void configure({notes_seconds:refreshRates[Number(e.target.value)]})}/></label></Popover.Content></Popover.Root></div>
          <NotesScroll key={meeting.id} version={noteText}>{meeting.notes_need_review && <p className="cl-error">Notes need refresh or review before sharing after your edits.</p>}{meeting.notes_error && <p className="cl-error" role="alert">{meeting.notes_error}</p>}{!meeting.notes && !meeting.notes_draft ? <div className="cl-empty"><p>Decisions, next steps, and useful details will collect here.</p>{active && <span>Notes start after the first completed turn and refresh as the conversation continues.</span>}</div> : <NotesOutline sections={noteDocument.sections} state={outlineState} toggle={(node_id,expanded)=>outlineAction({operation:"set",node_id,expanded})}/>}{!active && meeting.notes_revision < meeting.revision && <p className="cl-field-hint">Speaker labels or text changed. Refresh notes to update them.</p>}</NotesScroll>
        </section>}{!prefs.transcript_visible&&!prefs.notes_visible&&<div className="cl-empty"><p>Choose Transcript or Notes in View.</p></div>}</div>
        <PlaybackBar state={playback} set={play} close={()=>player.current?.close()}/>
        {active && <div className="cl-sourcebar"><span className="cl-recording-time">{stamp(Math.max(levels.duration,meeting.duration))}</span><Meter label="ME" value={meeting.status === 'recording' ? levels.me : 0} /><Meter label="Mac audio" value={meeting.status === 'recording' ? levels.remote : 0} remote /><span className="cl-muted">{Object.values(meeting.connection).some(v => v !== 'Connected' && v !== 'Stopped') ? Object.entries(meeting.connection).map(([k,v]) => `${k === 'me' ? 'Mic' : 'Mac'}: ${v}`).join(' · ') : 'Audio saved on this Mac'}</span><div className="cl-recording-controls">{recording && <button className="cl-button" disabled={busy} onClick={() => void action(() => invoke('cloud_pause', { paused: meeting.status !== 'paused' }))}>{meeting.status === 'paused' ? <Play size={16} /> : <Pause size={16} />}{meeting.status === 'paused' ? 'Resume' : 'Pause'}</button>}{active && meeting.status !== 'finishing' && <button className="cl-button cl-stop" disabled={busy} onClick={() => void action(() => invoke('cloud_stop'))}><Square size={14} fill="currentColor" /> Stop</button>}</div></div>}
        {active && meeting.capture && <div className="cl-capture-health" role="status">Local audio checkpoint: {stamp(meeting.capture.durable_seconds)}{meeting.recovery_needed?' · Live transcript needs recovery':''}</div>}
      </>}
    </main>{calendarOpen&&<CalendarPanel state={calendar.state} loadError={calendar.error} meeting={meeting} close={()=>setCalendarOpen(false)} changed={m=>{setMeeting(m);void refreshHistory();}}/>}{chatOpen&&<><ResizeDivider label="Chat width" reverse value={prefs.chat_width} min={280} max={700} preview={chat_width=>setPrefs(p=>({...p,chat_width}))} change={chat_width=>void configure({chat_width})}/><aside className="cl-chat-container" style={{width:prefs.chat_width}}><LibraryChat meeting={meeting} play={play} close={()=>setChatOpen(false)}/></aside></>}{settingsOpen && <LibrarySettings close={()=>setSettingsOpen(false)}/>} {meeting && editor && <EditDialog key={`${meeting.id}:${editor.kind}:${String(editor.turn_id??'')}`} meeting={meeting} edit={editor} close={()=>setEditor(null)} saved={m=>{setMeeting(m);void refreshHistory();}}/>}{meeting && sharing && <ShareDialog meeting={meeting} close={()=>setSharing(false)}/>} {connections && <Connections close={() => setConnections(false)} changed={setKeys} />}
  </div>;
}
