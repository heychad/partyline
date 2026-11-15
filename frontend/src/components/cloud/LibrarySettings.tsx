'use client';
import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { X } from 'lucide-react';
type Settings={automatic:boolean;folder:string|null};
type Backup={meeting_id:string;title?:string;status:string;progress:number;destination:string|null;error:string|null};
export default function LibrarySettings({close}:{close:()=>void}) {
  const [settings,setSettings]=useState<Settings>({automatic:true,folder:null});const [backups,setBackups]=useState<Backup[]>([]);const [busy,setBusy]=useState(false);const [error,setError]=useState('');const [notice,setNotice]=useState('');
  useEffect(()=>{let disposed=false;const removers:(()=>void)[]=[];async function init(){setSettings(await invoke('cloud_backup_settings'));setBackups((await invoke<{items:Backup[]}>('cloud_backup_status')).items);for(const event of ['cloud-backup','cloud-backup-settings']){const off=await listen(event,()=>{void invoke<Settings>('cloud_backup_settings').then(setSettings);void invoke<{items:Backup[]}>('cloud_backup_status').then(r=>setBackups(r.items));});if(disposed)off();else removers.push(off);}}void init().catch(e=>setError(String(e)));return()=>{disposed=true;removers.forEach(f=>f());};},[]);
  async function run(work:()=>Promise<void>){setBusy(true);setError('');setNotice('');try{await work();}catch(e){setError(typeof e==='string'?e:'Could not update settings. Try again.');}finally{setBusy(false);}}
  return <div className="cl-modal-backdrop"><section className="cl-modal cl-settings" role="dialog" aria-modal="true" aria-label="PartyLine settings"><div className="cl-heading"><h2>Settings</h2><button className="cl-icon" aria-label="Close settings" onClick={close}><X size={18}/></button></div>
    <h3>Google Drive backup</h3><p className="cl-field-hint">Choose a folder in Google Drive for desktop. Local originals stay in PartyLine; backups include the current trims and redactions. Existing backup versions remain in Drive.</p>
    <p className="cl-folder">{settings.folder || 'No destination selected'}</p><div className="cl-actions"><button className="cl-button" disabled={busy} onClick={()=>void run(async()=>{const result=await invoke<{cancelled:boolean;settings?:Settings}>('cloud_backup_choose_folder');if(result.settings)setSettings(result.settings);})}>Choose folder…</button>{settings.folder && <button className="cl-text-button" disabled={busy} onClick={()=>void run(async()=>{setSettings(await invoke('cloud_backup_configure',{automatic:settings.automatic,folder:null}));})}>Disconnect folder</button>}</div>
    <label className="cl-check"><input type="checkbox" checked={settings.automatic} disabled={busy} onChange={e=>void run(async()=>{setSettings(await invoke('cloud_backup_configure',{automatic:e.target.checked,folder:settings.folder}));})}/> Automatically back up finished recordings and edits</label>
    {!settings.folder && <p className="cl-field-hint">Backup starts only after you choose a folder.</p>}
    <p className="cl-field-hint">“Copied” means the files reached your Drive folder. Google Drive handles the cloud upload.</p>
    <div className="cl-backup-list">{backups.map(b=><div key={b.meeting_id}><strong>{b.title || 'Recording'}</strong><br/><span>{b.status==='copied'?'Copied to Drive folder':b.status} {b.status==='copying'?`${b.progress}%`:''}</span>{b.error && <p className="cl-error">{b.error}</p>}{['failed','interrupted'].includes(b.status) && <button className="cl-text-button" onClick={()=>void run(async()=>{await invoke('cloud_backup_start',{id:b.meeting_id});})}>Retry backup</button>}</div>)}</div>
    <h3>Agent connections</h3><p className="cl-field-hint">Connect a client, then start a new chat there. PartyLine must be open; its saved agent access setting applies.</p>
    <div className="cl-client-buttons">{[['codex','Connect Codex'],['claude-code','Connect Claude Code'],['claude-desktop','Connect Claude Desktop']].map(([client,label])=><button key={client} className="cl-button" disabled={busy} onClick={()=>void run(async()=>{await invoke('cloud_client_setup',{client});setNotice(`${label.replace('Connect ','')} configured. Restart that client or start a new chat to load PartyLine.`);})}>{label}</button>)}</div>
    {error && <p className="cl-error" role="alert">{error}</p>}{notice && <p className="cl-success" role="status">{notice}</p>}
  </section></div>;
}
