import type { Row } from './types';

export type TranscriptParagraph = { id: string; fragments: Row[] };
export type SpeakerGroup = { id: string; speaker: string; start: number; end: number; paragraphs: TranscriptParagraph[] };
const sentenceEnd = /[.!?]["'’”)]*\s*$/;

// Presentation only. Each fragment retains its original turn and audio range.
// Unfinished sentences cross provider turns; a pause or a few sentences starts
// the next paragraph. No language model rewrites the transcript.
export function transcriptGroups(rows: Row[]): SpeakerGroup[] {
  const groups: SpeakerGroup[] = [];
  for (const row of rows) {
    if (!row.text.trim()) continue;
    let group = groups.at(-1);
    if (!group || group.speaker !== row.speaker) {
      group = { id: row.id, speaker: row.speaker, start: row.start, end: row.end, paragraphs: [] };
      groups.push(group);
    }
    // Split long provider turns at sentence boundaries. Word timestamps, when
    // available, keep passage playback aligned with the original audio.
    const chunks: Row[] = [];
    let start = 0;
    const boundaries = [...row.text.matchAll(/[.!?]+["'’”)]*(?=\s|$)/g)].map(m => m.index! + m[0].length);
    boundaries.push(row.text.length);
    for (const end of [...new Set(boundaries)]) {
      const text = row.text.slice(start, end).trim();
      if (!text) continue;
      const before = row.text.slice(0, start).trim().split(/\s+/).filter(Boolean).length;
      const count = text.split(/\s+/).length;
      chunks.push({ ...row, id: `${row.id}@${start}`, text, start: row.words?.[before]?.start ?? row.start, end: row.words?.[before + count - 1]?.end ?? row.end });
      start = end;
    }
    for (const fragment of chunks) {
      let paragraph = group.paragraphs.at(-1);
      const previous = paragraph?.fragments.at(-1);
      const words = paragraph?.fragments.reduce((n, f) => n + f.text.split(/\s+/).length, 0) ?? 0;
      if (!paragraph || (previous && sentenceEnd.test(previous.text) && (words >= 45 || fragment.start - previous.end >= 1.2))) {
        paragraph = { id: fragment.id, fragments: [] };
        group.paragraphs.push(paragraph);
      }
      paragraph.fragments.push(fragment);
    }
    group.end = Math.max(group.end, row.end);
  }
  return groups;
}

export function transcriptPage(groups: SpeakerGroup[], offset = 0, limit = 50) {
  if (!Number.isInteger(offset) || offset < 0 || !Number.isInteger(limit) || limit < 1 || limit > 100) throw new Error('Choose a valid transcript page.');
  return { items: groups.slice(offset, offset + limit), total: groups.length, offset, next_offset: offset + limit < groups.length ? offset + limit : null };
}

// One edit action per source turn, spanning every displayed fragment of it.
export function paragraphSources(paragraph: TranscriptParagraph): Row[] {
  const sources = new Map<string, Row>();
  for (const row of paragraph.fragments) {
    const previous = sources.get(row.turn_id);
    sources.set(row.turn_id, previous ? { ...previous, text: `${previous.text} ${row.text}`, start: Math.min(previous.start, row.start), end: Math.max(previous.end, row.end) } : { ...row });
  }
  return [...sources.values()];
}
