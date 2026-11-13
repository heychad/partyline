'use client';
import { useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';
import type { Meeting } from '../cloud/types';

export type KeyStatus = { assemblyai: boolean; openai: boolean };
export type Draft = { title?: string; participants?: string[]; microphone?: string };
export type AgentView = {
  selected_id: string | null; follow: boolean; connections_open: boolean;
  draft: { title: string; participants: string[]; microphone: string };
};
export type AgentUiRequest = { id: string; action: string; arguments: Record<string, unknown> };
export type AgentUiHandlers = {
  view: AgentView; busy: boolean;
  open: (id: string) => Promise<void>;
  newMeeting: () => void;
  prepare: (draft: Draft) => void;
  setFollow: (enabled: boolean) => void;
  setConnections: (open: boolean) => void;
  dismiss: (target: 'error' | 'notice') => void;
  copied: () => void;
  keysChanged: (status: KeyStatus) => void;
};

// Kept separate so view behavior can be tested without a native window.
export async function applyAgentUiRequest(
  request: AgentUiRequest,
  handlers: AgentUiHandlers,
  current: () => Promise<Meeting | null>,
): Promise<void> {
  if (handlers.busy) throw new Error('The window is busy.');
  const args = request.arguments;
  if (['new', 'prepare', 'open'].includes(request.action)) {
    const active = await current();
    if (active && (request.action !== 'open' || args.id !== active.id)) {
      throw new Error('Finish the active meeting first.');
    }
  }
  switch (request.action) {
    case 'open': await handlers.open(String(args.id)); break;
    case 'new': handlers.newMeeting(); break;
    case 'prepare': handlers.prepare(args as Draft); break;
    case 'follow': handlers.setFollow(args.enabled === true); break;
    case 'connections': handlers.setConnections(args.open === true); break;
    case 'dismiss':
      if (args.target !== 'error' && args.target !== 'notice') throw new Error('Unknown banner.');
      handlers.dismiss(args.target); break;
    case 'copied': handlers.copied(); break;
    default: throw new Error('Unsupported window action.');
  }
}

export function useCloudAgentBridge(handlers: AgentUiHandlers) {
  const latest = useRef(handlers);
  latest.current = handlers;
  useEffect(() => {
    let disposed = false;
    const listeners: UnlistenFn[] = [];
    async function attach() {
      const off = await listen<AgentUiRequest>('partyline-agent-ui', ({ payload }) => {
        void applyAgentUiRequest(payload, latest.current, () => invoke<Meeting | null>('cloud_current'))
          .then(() => true, () => false)
          .then(ok => invoke('plugin:agent-control|ui_result', { id: payload.id, ok }))
          .catch(() => {});
      });
      if (disposed) { off(); return; }
      listeners.push(off);
      const keys = await listen<KeyStatus>('partyline-agent-connections', ({ payload }) => latest.current.keysChanged(payload));
      if (disposed) { keys(); return; }
      listeners.push(keys);
      await invoke('plugin:agent-control|ui_state', { view: { ...latest.current.view, ready: true } });
    }
    void attach().catch(() => {});
    return () => {
      disposed = true;
      listeners.forEach(off => off());
      void invoke('plugin:agent-control|ui_state', { view: { ...latest.current.view, ready: false } }).catch(() => {});
    };
  }, []);
  const snapshot = JSON.stringify(handlers.view);
  useEffect(() => {
    void invoke('plugin:agent-control|ui_state', { view: { ...JSON.parse(snapshot), ready: true } }).catch(() => {});
  }, [snapshot]);
}
