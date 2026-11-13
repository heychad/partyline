export type Word = { text: string; start: number; end: number; speaker: string };
export type Turn = { id: string; channel: string; order: number; text: string; start: number; end: number; speaker: string; final_text: boolean; words: Word[] };
export type Meeting = {
  id: string; title: string; created_at: string; status: string; duration: number;
  turns: Record<string, Turn>; names: Record<string, string>; participants: string[];
  notes: string; notes_draft: string; notes_status: string; notes_error: string | null;
  connection: Record<string, string>; errors: string[]; revision: number; notes_revision: number;
};
export type Row = { id: string; text: string; start: number; speaker: string; final: boolean };
export const isActive = (m: Meeting | null) => !!m && ['connecting', 'recording', 'paused', 'finishing'].includes(m.status);
export const stamp = (seconds: number) => `${Math.floor(seconds / 60).toString().padStart(2, '0')}:${Math.floor(seconds % 60).toString().padStart(2, '0')}`;
export const pending = (id: string) => id.endsWith(':PENDING') || id.endsWith(':UNKNOWN');
export function speakerName(m: Meeting, id: string): string {
  if (id === 'me') return 'ME';
  if (m.names[id]) return m.names[id];
  if (pending(id)) return 'Identifying speaker…';
  const [, generation, label] = id.split(':');
  return `Speaker ${label}${generation !== '0' ? ` · connection ${Number(generation) + 1}` : ''}`;
}
export function transcriptRows(m: Meeting): Row[] {
  const rows: Row[] = [];
  for (const t of Object.values(m.turns)) {
    if (!t.words.length || !t.final_text) { rows.push({ id: t.id, text: t.text, start: t.start, speaker: t.speaker, final: t.final_text }); continue; }
    let row: Row | null = null;
    t.words.forEach((w, i) => {
      if (!row || row.speaker !== w.speaker) {
        row = { id: `${t.id}:${i}`, text: w.text, start: w.start, speaker: w.speaker, final: t.final_text }; rows.push(row);
      } else row.text += ` ${w.text}`;
    });
  }
  return rows.sort((a, b) => a.start - b.start || a.id.localeCompare(b.id));
}
