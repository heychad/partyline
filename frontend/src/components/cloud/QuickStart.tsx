import { Loader2, Mic, Monitor } from 'lucide-react';
export default function QuickStart({ title, participants, microphone, devices, busy, start, selectMicrophone, connections }: {
  title: string; participants: string[]; microphone: string; devices: {name:string}[]; busy: boolean;
  start: () => void; selectMicrophone: (value:string) => void; connections: () => void;
}) {
  return <section className="cl-quick-start">
    <h1>{title || 'Ready to record.'}</h1>
    {participants.length > 0 && <p className="cl-muted">{participants.join(', ')}</p>}
    <button className="cl-button cl-primary cl-quick-record" disabled={busy} onClick={start}>{busy ? <Loader2 size={18} className="cl-spin"/> : <span className="cl-record-dot"/>}Start recording</button>
    <div className="cl-quick-inputs"><Mic size={15}/><select aria-label="Microphone" value={microphone} onChange={e=>selectMicrophone(e.target.value)}><option value="">System default</option>{devices.map(d=><option key={d.name} value={d.name}>{d.name}</option>)}</select><span><Monitor size={15}/>Mac audio</span></div>
    <p className="cl-field-hint">Add a title and participants while you record.</p>
    <button className="cl-text-button" onClick={connections}>Connections & audio check</button>
  </section>;
}
