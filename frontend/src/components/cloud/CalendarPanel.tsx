import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { CalendarDays, X, RefreshCw } from 'lucide-react';
import type { Meeting } from './types';
export type CalendarEvent={key:string;account_id:string;calendar_id:string;event_id:string;title:string;start:string;end:string;participants:string[];meeting_url:string|null};
type Account={id:string;email:string;selected:string[];calendars:{id:string;name:string;primary:boolean}[]};
export type CalendarState={configured:boolean;accounts:Account[];events:CalendarEvent[];account_errors:Record<string,string>;phase:string;error:string|null;last_sync:string|null;detection:{enabled:boolean;signals:{id:string;app:string;evidence:string}[];error:string|null;last_scan:string|null}};
export const calendarAction=(request:Record<string,unknown>)=>invoke<CalendarState>('cloud_calendar_action',{request});
export function useCalendar(){
 const [state,setState]=useState<CalendarState|null>(null);const [error,setError]=useState('');
 useEffect(()=>{let disposed=false;let off:UnlistenFn|undefined;void(async()=>{off=await listen<CalendarState>('cloud-calendar',({payload})=>{if(!disposed)setState(payload);});if(disposed){off();return;}const result=await calendarAction({action:'status'});if(!disposed)setState(result);})().catch(e=>{if(!disposed)setError(String(e));});return()=>{disposed=true;off?.();};},[]);
 return {state,error};
}
export function upcoming(events:CalendarEvent[],now=Date.now()){return events.filter(e=>Date.parse(e.end)>now&&Date.parse(e.start)<now+24*3600000).sort((a,b)=>Date.parse(a.start)-Date.parse(b.start));}
const time=(date:string)=>new Date(date).toLocaleString(undefined,{weekday:'short',hour:'numeric',minute:'2-digit'});
export function CalendarPrompt({state,active,start,open,busy}:{state:CalendarState|null;active:boolean;start:()=>void;open:()=>void;busy:boolean}){
 if(!state)return null;const signal=state.detection.signals[0];
 if(active||!signal)return null;
 return <div className="cl-call-prompt" role="status"><CalendarDays size={18}/><span>{signal.evidence}</span><button className="cl-button cl-primary" disabled={busy} onClick={start}>Record</button><button className="cl-text-button" onClick={open}>Calendar</button><button className="cl-icon" aria-label="Dismiss call suggestion" onClick={()=>void calendarAction({action:'dismiss',signal_id:signal.id}).catch(()=>open())}><X size={16}/></button></div>;
}
export default function CalendarPanel({state,loadError,meeting,close,changed}:{state:CalendarState|null;loadError:string;meeting:Meeting|null;close:()=>void;changed:(m:Meeting)=>void}){
 const [error,setError]=useState('');const [busy,setBusy]=useState(false);const [clientId,setClientId]=useState('');const [clientSecret,setClientSecret]=useState('');const [details,setDetails]=useState(true);
 const waiting=state?.phase!=='idle';const disabled=busy||waiting;
 async function run(work:()=>Promise<unknown>){setBusy(true);setError('');try{await work();}catch(e){setError(String(e));}finally{setBusy(false);}}
 const action=(request:Record<string,unknown>)=>void run(()=>calendarAction(request));
 async function link(event:CalendarEvent|null){if(!meeting)return;await run(async()=>changed(await invoke<Meeting>('cloud_calendar_link',{id:meeting.id,expectedVersion:meeting.edit_version??0,eventKey:event?.key??null,useDetails:!!event&&details})));}
 return <aside className="cl-calendar-panel" aria-label="Calendar and meeting detection"><div className="cl-heading"><h2>Calendar</h2><button className="cl-icon" aria-label="Close calendar" onClick={close}><X size={19}/></button></div>
  {(error||loadError||state?.error)&&<p className="cl-error" role="alert">{error||loadError||state?.error}</p>}
  {!state?<p>Loading calendars…</p>:<>
  <section className="cl-calendar-section"><h3>Google accounts</h3>
   {(!state.configured||!state.accounts.length)&&<form onSubmit={e=>{e.preventDefault();void run(async()=>{await calendarAction({action:'configure_client',client_id:clientId,client_secret:clientSecret});setClientId('');setClientSecret('');});}}><p className="cl-muted">One-time setup: import your Google Desktop app OAuth client.</p><button type="button" className="cl-button" disabled={disabled} onClick={()=>action({action:'import_client'})}>Import client JSON</button><label>Client ID<input value={clientId} onChange={e=>setClientId(e.target.value)} autoComplete="off" maxLength={512}/></label><label>Client secret<input type="password" value={clientSecret} onChange={e=>setClientSecret(e.target.value)} autoComplete="off" maxLength={512}/></label><button className="cl-button" disabled={disabled||!clientId||!clientSecret}>Save OAuth client</button></form>}
   {state.accounts.map(a=><div key={a.id} className="cl-calendar-account"><strong>{a.email}</strong>{state.account_errors[a.id]&&<p className="cl-error" role="alert">{state.account_errors[a.id]}</p>}
    {a.calendars.map(c=>{const id=c.primary?'primary':c.id;return <label key={c.id} className="cl-check"><input type="checkbox" disabled={disabled} checked={a.selected.includes(id)||a.selected.includes(c.id)} onChange={e=>action({action:'select',account_id:a.id,calendar_ids:e.target.checked?[...a.selected.filter(v=>v!==c.id&&v!==id),id]:a.selected.filter(v=>v!==c.id&&v!==id)})}/>{c.name}</label>;})}
    {!a.calendars.length&&<p className="cl-muted">Refresh to load calendars.</p>}
    <button className="cl-text-button" disabled={disabled} onClick={()=>action({action:'connect',account_id:a.id})}>Reconnect</button><button className="cl-text-button" disabled={disabled} onClick={()=>action({action:'disconnect',account_id:a.id})}>Remove account</button>
   </div>)}
   <div className="cl-actions"><button className="cl-button" disabled={!state.configured||disabled} onClick={()=>action({action:'connect'})}>Add Google account</button><button className="cl-icon" aria-label="Refresh calendars" disabled={disabled||!state.accounts.length} onClick={()=>action({action:'refresh'})}><RefreshCw size={17}/></button></div>
   {waiting&&<p role="status">{state.phase==='awaiting_google'?'Finish Google sign-in in your browser.':state.phase==='refreshing'?'Refreshing calendars…':state.phase==='cancelling'?'Cancelling sign-in…':'Saving connection…'}{['awaiting_google','cancelling'].includes(state.phase)&&<button className="cl-text-button" disabled={busy||state.phase==='cancelling'} onClick={()=>action({action:'cancel'})}>Cancel sign-in</button>}</p>}
   <p className="cl-field-hint">Read-only access. Selected event details stay local until you attach them to a recording. Invitees are name suggestions.</p>
  </section>
  <section className="cl-calendar-section"><h3>Meeting detection</h3><label className="cl-check"><input type="checkbox" checked={state.detection.enabled} disabled={disabled} onChange={e=>action({action:'detection_configure',enabled:e.target.checked})}/>Suggest recording when a call app uses the microphone</label><p className="cl-field-hint">Zoom, Teams, Slack, FaceTime, Webex, and supported browsers. Browser activity may be a call. You choose when to record.</p>{state.detection.error&&<p className="cl-error" role="alert">{state.detection.error}</p>}</section>
  <section className="cl-calendar-section"><h3>{meeting?'Link this recording':'Upcoming'}</h3>
   {meeting?.calendar_event&&<div className="cl-calendar-linked"><strong>{meeting.calendar_event.title}</strong><span>{time(meeting.calendar_event.start)}</span><button className="cl-text-button" disabled={busy} onClick={()=>void link(null)}>Unlink event</button></div>}
   {meeting&&<label className="cl-check"><input type="checkbox" checked={details} onChange={e=>setDetails(e.target.checked)}/>Use event title and invitees</label>}
   {!meeting&&<p className="cl-field-hint">Start recording first, then attach an event here.</p>}
   {(meeting?state.events:upcoming(state.events)).map(e=><article className="cl-calendar-event" key={e.key}><strong>{e.title}</strong><span>{time(e.start)} · {state.accounts.find(a=>a.id===e.account_id)?.email}</span><small>{e.participants.length} invitees{e.meeting_url?' · Online meeting':''}</small>{meeting&&<button className="cl-button" disabled={busy||!!meeting.deleted_at||meeting.status==='finishing'||meeting.calendar_event?.key===e.key} onClick={()=>void link(e)}>{meeting.calendar_event?.key===e.key?'Linked':'Attach event'}</button>}</article>)}
   {!state.events.length&&<p className="cl-muted">No timed events in your selected calendars.</p>}
   {state.last_sync&&<p className="cl-field-hint">Last refreshed {time(state.last_sync)}. Past 12 hours and next 7 days.</p>}
  </section></>}
 </aside>;
}
