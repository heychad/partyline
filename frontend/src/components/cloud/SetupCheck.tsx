import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { ShieldCheck, Loader2 } from 'lucide-react';
type Setup = { status: string; phase: string; error?: string; microphone_received: boolean; mac_audio_received: boolean; microphone_signal: boolean; mac_audio_signal: boolean; connections_unlocked: boolean };
export default function SetupCheck({ microphone = '', busyChanged }: { microphone?: string; busyChanged?: (busy: boolean) => void }) {
  const [state, setState] = useState<Setup | null>(null), [error, setError] = useState('');
  useEffect(() => {
    let disposed = false, off: UnlistenFn | undefined;
    void (async () => {
      off = await listen<Setup>('cloud-setup', ({ payload }) => { if (!disposed) setState(payload); });
      if (disposed) { off(); return; }
      const value = await invoke<Setup>('cloud_setup_status'); if (!disposed) setState(value);
    })().catch(e => { if (!disposed) setError(String(e)); });
    return () => { disposed = true; off?.(); };
  }, []);
  const running = state?.status === 'running';
  useEffect(() => { busyChanged?.(running); }, [running, busyChanged]);
  async function start() { setError(''); try { setState(await invoke<Setup>('cloud_setup_start', { microphone: microphone || null })); } catch (e) { setError(String(e)); } }
  return <div className="cl-setup-check"><div className="cl-actions"><button type="button" className="cl-button" disabled={running} onClick={() => void start()}>{running ? <Loader2 size={15} className="cl-spin" /> : <ShieldCheck size={15} />}Check permissions & audio</button>{running && <button type="button" className="cl-text-button" onClick={() => void invoke('cloud_setup_cancel').catch(e => setError(String(e)))}>Cancel check</button>}</div>
    <p className="cl-field-hint">Approve macOS access here before your meeting. The eight-second check discards audio; nothing is uploaded.</p>
    {running && <p className="cl-field-hint" role="status">{state?.phase === 'unlocking' ? 'Unlocking connections. Approve Keychain prompts with Always Allow.' : 'Speak and play some Mac audio to check both inputs.'}</p>}
    {state?.status === 'complete' && <p className="cl-field-hint" role="status">Mic: {state.microphone_signal ? 'signal received' : state.microphone_received ? 'silent' : 'no samples'}. Mac audio: {state.mac_audio_signal ? 'signal received' : state.mac_audio_received ? 'silent' : 'no samples'}. {(!state.microphone_signal || !state.mac_audio_signal) && 'Silence does not confirm permission—play audio or speak, then check again.'}</p>}
    {state?.status === 'cancelled' && <p className="cl-field-hint">Setup check cancelled.</p>}
    {(error || state?.error) && <p className="cl-error" role="alert">{error || state?.error}</p>}
  </div>;
}
