export type Word = { text: string; start: number; end: number; speaker: string };
export type Turn = { id: string; channel: string; order: number; text: string; start: number; end: number; speaker: string; final_text: boolean; words: Word[] };
export type Meeting = {
  calendar_event?: import("./CalendarPanel").CalendarEvent | null;
  capture?:{recording:boolean;durable_seconds:number;live_skipped_seconds:number[]};recovery_needed?:boolean;
  recovery?:{status:string;phase:string;channel:string;progress:number;error:string|null};
  speaker_aliases?:Record<string,string>;speaker_edit_version?:number;previous_aliases?:Record<string,string>|null;
  id: string; title: string; created_at: string; status: string; duration: number;
  turns: Record<string, Turn>; names: Record<string, string>; participants: string[];
  edit_version?: number; undo_version?: number | null; trim_start?: number; trim_end?: number | null; redactions?: { id: string; start: number; end: number }[]; deleted_at?: string | null; notes_need_review?: boolean;
  notes: string; notes_draft: string; notes_status: string; notes_error: string | null;
  connection: Record<string, string>; errors: string[]; revision: number; notes_revision: number;
};
export type Row = { id: string; turn_id: string; text: string; start: number; end: number; channel: string; speaker: string; final: boolean; words?: Word[] };
export const isActive = (m: Meeting | null) => !!m && ['connecting', 'recording', 'paused', 'finishing'].includes(m.status);
export const stamp = (seconds: number) => `${Math.floor(seconds / 60).toString().padStart(2, '0')}:${Math.floor(seconds % 60).toString().padStart(2, '0')}`;
export const pending = (id: string) => id.endsWith(':PENDING') || id.endsWith(':UNKNOWN');
export function speakerName(m: Meeting, id: string): string {
  id=m.speaker_aliases?.[id]??id;
  if (id === 'me') return 'ME';
  if (m.names[id]) return m.names[id];
  if (pending(id)) return 'Identifying speaker…';
  const [, generation, label] = id.split(':');
  if(generation.startsWith('recovered-'))return `Recovered speaker ${label}`;
  return `Speaker ${label}${generation !== '0' ? ` · connection ${Number(generation) + 1}` : ''}`;
}
export function transcriptRows(m: Meeting): Row[] {
  const rows: Row[] = [];
  for (const t of Object.values(m.turns)) {
    if (!t.words.length || !t.final_text) { rows.push({ id: t.id, turn_id: t.id, text: t.text, start: t.start, end: t.end, channel: t.channel, speaker: m.speaker_aliases?.[t.speaker]??t.speaker, final: t.final_text }); continue; }
    let row: Row | null = null;
    t.words.forEach((w, i) => {
      const speaker=m.speaker_aliases?.[w.speaker]??w.speaker;
      if (!row || row.speaker !== speaker) {
        row = { id: `${t.id}:${i}`, turn_id: t.id, text: w.text, start: w.start, end: w.end, channel: t.channel, speaker, final: t.final_text, words: [w] }; rows.push(row);
      } else { row.text += ` ${w.text}`; row.end = w.end; row.words!.push(w); }
    });
  }
  return rows.sort((a, b) => a.start - b.start || a.id.localeCompare(b.id));
}
