'use client';
import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';
import './agent-control.css';

type Access = 'off' | 'read' | 'control';
type Activity = { sequence: number; timestamp_ms: number; label: string; outcome: string };
type State = { access: Access; activity: Activity[]; error: string | null };
const labels = { off: 'Off', read: 'Read only', control: 'Control' };

export default function AgentAccess() {
  const [state, setState] = useState<State>({ access: 'off', activity: [], error: null });
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let disposed = false;
    const listeners: UnlistenFn[] = [];
    async function attach() {
      const activity = await listen<Activity>('partyline-agent-activity', ({ payload }) => {
        setState(s => ({ ...s, activity: [...s.activity.filter(a => a.sequence !== payload.sequence), payload]
          .sort((a, b) => a.sequence - b.sequence).slice(-100) }));
      });
      if (disposed) { activity(); return; } listeners.push(activity);
      const access = await listen<Access>('partyline-agent-permissions', ({ payload }) => setState(s => ({ ...s, access: payload })));
      if (disposed) { access(); return; } listeners.push(access);
      const unavailable = await listen<string>('partyline-agent-unavailable', ({ payload }) => setState(s => ({ ...s, error: payload })));
      if (disposed) { unavailable(); return; } listeners.push(unavailable);
      const value = await invoke<State>('plugin:agent-control|permissions');
      if (!disposed) setState(value);
    }
    void attach().catch(() => { if (!disposed) setState(s => ({ ...s, error: 'Agent control is unavailable in this build.' })); });
    return () => { disposed = true; listeners.forEach(off => off()); };
  }, []);
  async function change(access: Access) {
    setBusy(true);
    try {
      await invoke('plugin:agent-control|set_permissions', { access });
      const value = await invoke<State>('plugin:agent-control|permissions');
      setState(value);
    } catch { setState(s => ({ ...s, error: 'Could not change agent access. Try again.' })); }
    finally { setBusy(false); }
  }
  return <details className="ag-access">
    <summary>Agents · {labels[state.access]}</summary>
    <section className="ag-panel" aria-label="Agent access">
      <label htmlFor="agent-access">Agent access</label>
      <select id="agent-access" value={state.access} disabled={busy} onChange={e => void change(e.target.value as Access)}>
        <option value="off">Off</option><option value="read">Read only</option><option value="control">Control</option>
      </select>
      <p>Applies to agents on this Mac account until PartyLine quits. Control allows recording, exports, and connection changes.</p>
      {state.error && <p role="alert" className="ag-error">{state.error}</p>}
      <div className="ag-activity" role="log" aria-label="Agent activity" aria-live="polite">
        {state.activity.length ? [...state.activity].reverse().map(item => <div key={item.sequence}>
          <span>{item.label}</span><small>{item.outcome} · {new Date(item.timestamp_ms).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</small>
        </div>) : <p>No agent activity yet.</p>}
      </div>
    </section>
  </details>;
}
