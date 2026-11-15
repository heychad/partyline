'use client';
import { Pause, Play, SkipBack, SkipForward, X } from 'lucide-react';
import { PlaybackRequest, PlaybackState } from './playback';
import { stamp } from './types';
export default function PlaybackBar({ state, set, close }: { state: PlaybackState; set: (request: PlaybackRequest) => Promise<void>; close: () => void }) {
  if (!state.id) return null;
  const change = (patch: Partial<PlaybackRequest>) => { void set({ id: state.id!, ...patch }).catch(() => {}); };
  const loading = state.status === 'loading';
  return <section className="cl-playback" aria-label="Recorded audio">
    <button className="cl-icon" aria-label="Back 10 seconds" disabled={loading} onClick={() => change({ position: Math.max(state.start,state.position-10) })}><SkipBack size={16}/></button>
    <button className="cl-button" aria-label={state.status === 'playing' ? 'Pause playback' : 'Play recording'} disabled={loading} onClick={() => change({ playing: state.status !== 'playing', position: state.status === 'ended' ? state.start : state.position })}>{state.status === 'playing' ? <Pause size={16}/> : <Play size={16}/>} {loading ? 'Loading…' : state.status === 'playing' ? 'Pause' : 'Play'}</button>
    <button className="cl-icon" aria-label="Forward 10 seconds" disabled={loading} onClick={() => change({ position: Math.min(state.end,state.position+10) })}><SkipForward size={16}/></button>
    <span>{stamp(state.position)}</span><input aria-label="Playback position" type="range" min={state.start} max={state.end || 1} step="0.1" value={state.position} disabled={loading} onChange={e=>change({position:Number(e.target.value)})}/><span>{stamp(state.end)}</span>
    <select aria-label="Playback speed" value={state.speed} disabled={loading} onChange={e=>change({speed:Number(e.target.value)})}>{[0.5,0.75,1,1.25,1.5,1.75,2,2.5,3].map(rate=><option key={rate} value={rate}>{rate}×</option>)}</select>
    <select aria-label="Playback source" value={state.channel} disabled={loading} onChange={e=>change({channel:e.target.value})}><option value="both">Both sources</option><option value="me">ME</option><option value="remote">Mac audio</option></select>
    <button className="cl-icon" aria-label="Close playback" onClick={close}><X size={16}/></button>
    {state.error && <p className="cl-error" role="alert">{state.error}</p>}
  </section>;
}
