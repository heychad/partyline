export type AudioInfo = { meeting_id: string; path: string; start: number; end: number; duration: number; edit_version: number; channel: string };
export type PlaybackState = { id: string | null; status: 'idle' | 'loading' | 'playing' | 'paused' | 'ended' | 'error'; position: number; start: number; end: number; speed: number; channel: string; error: string | null };
export type PlaybackRequest = { id: string; position?: number; speed?: number; channel?: string; playing?: boolean };
export const emptyPlayback = (): PlaybackState => ({ id: null, status: 'idle', position: 0, start: 0, end: 0, speed: 1, channel: 'both', error: null });
export type AudioPort = Pick<HTMLAudioElement, 'src' | 'currentTime' | 'playbackRate' | 'paused' | 'readyState' | 'play' | 'pause' | 'load' | 'addEventListener' | 'removeEventListener'>;

// Human controls and agent requests call this same controller.
export class PlaybackController {
  state = emptyPlayback();
  private generation = 0;
  private observed: { id: string | null; version: number } | null = null;
  private audio: AudioPort;
  private resolve: (id: string, channel: string) => Promise<AudioInfo>;
  private source: (path: string) => string;
  private changed: (state: PlaybackState) => void;
  constructor(audio: AudioPort, resolve: (id: string, channel: string) => Promise<AudioInfo>, source: (path: string) => string, changed: (state: PlaybackState) => void) {
    this.audio=audio;this.resolve=resolve;this.source=source;this.changed=changed;
    audio.addEventListener('timeupdate', () => { if (this.state.id) this.update({ position: Math.min(this.state.end, this.state.start + audio.currentTime) }); });
    audio.addEventListener('ended', () => { if (this.state.id) this.update({ status: 'ended', position: this.state.end }); });
    audio.addEventListener('error', () => { if (this.state.id && this.state.status !== 'loading') this.update({ status: 'error', error: 'Audio could not play. Try reopening the recording.' }); });
  }
  private update(patch: Partial<PlaybackState>) { this.state = { ...this.state, ...patch }; this.changed({ ...this.state }); }
  observeRecording(id: string | null, version: number) {
    const previous = this.observed;
    this.observed = { id, version };
    // Opening a different recording must not cancel the playback request that opened it.
    if (previous?.id === id && previous.version !== version && this.state.id === id) this.close();
  }
  close() { ++this.generation; this.audio.pause(); this.audio.src = ''; this.state = emptyPlayback(); this.changed({ ...this.state }); }
  async set(request: PlaybackRequest) {
    const speed = request.speed ?? this.state.speed;
    const channel = request.channel ?? (request.id === this.state.id ? this.state.channel : 'both');
    if (!Number.isFinite(speed) || speed < 0.5 || speed > 3 || !['both','me','remote'].includes(channel)) throw new Error('Choose a speed from 0.5 to 3 and a valid audio channel.');
    if (request.position !== undefined && (!Number.isFinite(request.position) || request.position < 0)) throw new Error('Choose a valid playback time.');
    const reload = request.id !== this.state.id || channel !== this.state.channel || this.state.status === 'error';
    const previousPosition=request.id===this.state.id?this.state.position:undefined;
    const wasPlaying=this.state.status==='playing';
    const generation = ++this.generation;
    try {
      if (reload) {
        this.audio.pause(); this.update({ id: request.id, status: 'loading', channel, error: null });
        const info = await this.resolve(request.id, channel);
        if (generation !== this.generation) return;
        const loaded = new Promise<void>((resolve,reject) => {
          const clear = () => { clearTimeout(timer); this.audio.removeEventListener('loadedmetadata',ready); this.audio.removeEventListener('error',failed); };
          const ready = () => { clear(); resolve(); };
          const failed = () => { clear(); reject(new Error('Audio could not load. Reopen the recording to try again.')); };
          const timer = setTimeout(failed, 20000);
          this.audio.addEventListener('loadedmetadata',ready); this.audio.addEventListener('error',failed);
        });
        this.audio.src = this.source(info.path); this.audio.load();
        await loaded;
        if (generation !== this.generation) return;
        this.update({ start: info.start, end: info.end, position: info.start, status: 'paused' });
      }
      const position = request.position ?? previousPosition ?? this.state.position;
      if (position < this.state.start || position > this.state.end) throw new Error('That time is outside the trimmed recording.');
      this.audio.currentTime = position - this.state.start;
      this.audio.playbackRate = speed;
      this.update({ speed, position, error: null });
      const playing = request.playing ?? wasPlaying;
      if (playing) { await this.audio.play(); if (generation === this.generation) this.update({ status: 'playing' }); }
      else { this.audio.pause(); this.update({ status: 'paused' }); }
    } catch (error) {
      if (generation === this.generation) this.update({ status: 'error', error: error instanceof Error ? error.message : 'Playback failed. Try again.' });
      throw error;
    }
  }
}
