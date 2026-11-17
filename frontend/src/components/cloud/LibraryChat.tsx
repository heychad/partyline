'use client';
import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import ReactMarkdown from 'react-markdown';
import { X } from 'lucide-react';
import { Meeting } from './types';
import { ChatCitations, ChatComposer, ChatSource } from './ChatControls';
import { PlaybackRequest } from './playback';
type Reply={id:string;question:string;answer:string;status:string;phase?:string;error:string|null;sources:ChatSource[];cited_source_indices:number[];meetings_searched:number;passages_used:number;meeting_id:string|null};
export default function LibraryChat({close,meeting,play}:{close:()=>void;meeting:Meeting|null;play:(request:PlaybackRequest)=>Promise<void>}){
  const [question,setQuestion]=useState('');const [scope,setScope]=useState(meeting?'current':'all');const [replies,setReplies]=useState<Reply[]>([]);const [error,setError]=useState('');const [busy,setBusy]=useState(false);
  useEffect(()=>{let disposed=false;const removers:(()=>void)[]=[];const refresh=()=>invoke<{items:Reply[]}>('cloud_chat_list').then(r=>{if(!disposed)setReplies(r.items);});async function init(){await refresh();for(const event of ['cloud-chat','cloud-library']){const off=await listen(event,()=>void refresh().catch(e=>setError(String(e))));if(disposed)off();else removers.push(off);}}void init().catch(e=>setError(String(e)));return()=>{disposed=true;removers.forEach(f=>f());};},[]);
  const running=replies.find(r=>r.status==='running');
  async function ask(text:string,meetingId:string|null){setBusy(true);setError('');try{await invoke('cloud_chat_ask',{question:text,meetingId});setQuestion('');}catch(e){setError(String(e));}finally{setBusy(false);}}
  return <section className="cl-chat cl-chat-docked" aria-label="Chat with recordings"><div className="cl-heading"><h2>Ask your recordings</h2><button className="cl-icon" aria-label="Close recording chat" onClick={close}><X size={18}/></button></div>
    <div className="cl-chat-history">{!replies.length && <p className="cl-muted">Ask about decisions, commitments, or a topic across your calls.</p>}{replies.map(reply=><article key={reply.id}><h3>{reply.question}</h3><div className="cl-markdown"><ReactMarkdown>{reply.answer || (reply.status==='running'?(reply.phase==='unlocking'?'Unlocking the OpenAI connection. Approve any macOS Keychain prompt.':'Looking through your recordings…'):'')}</ReactMarkdown></div>{reply.error && <p className="cl-error">{reply.error}</p>}<ChatCitations sources={reply.sources} indices={reply.cited_source_indices??[]} open={source=>{void play({id:source.meeting_id,position:source.start,playing:true}).catch(e=>setError(String(e)));}}/>{['failed','interrupted','outdated','cancelled'].includes(reply.status) && <button className="cl-text-button" disabled={!!running||busy} onClick={()=>void ask(reply.question,reply.meeting_id)}>Ask again</button>}{reply.status==='running' && <button className="cl-text-button" onClick={()=>void invoke('cloud_chat_cancel',{id:reply.id}).catch(e=>setError(String(e)))}>Cancel answer</button>}</article>)}</div>
    <ChatComposer question={question} setQuestion={setQuestion} scope={scope} setScope={setScope} currentAvailable={!!meeting&&!meeting.deleted_at} busy={busy||!!running} error={error} submit={()=>void ask(question,scope==='current'?meeting?.id??null:null)}/>

  </section>;
}
