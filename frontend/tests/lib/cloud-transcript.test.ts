import { describe, expect, test } from 'bun:test';
import { Meeting, speakerName, transcriptRows } from '../../src/components/cloud/types';
const meeting = (): Meeting => ({ id: 'test', title: 'Test', created_at: '', status: 'recording', duration: 0, turns: {}, names: { me: 'ME' }, participants: ['Nicole'], notes: '', notes_draft: '', notes_status: '', notes_error: null, connection: {}, errors: [], revision: 0, notes_revision: 0 });
describe('cloud speaker rendering', () => {
  test('splits mixed-speaker turns, sorts the two sources, and applies confirmed names', () => {
    const m = meeting(); m.names['remote:0:B'] = 'Nicole';
    m.turns.remote = { id: 'remote:0:0', channel: 'remote', order: 0, text: 'Hello. Hi!', start: 2, end: 4, speaker: 'remote:0:A', final_text: true, words: [{ text: 'Hello.', start: 2, end: 3, speaker: 'remote:0:A' }, { text: 'Hi!', start: 3, end: 4, speaker: 'remote:0:B' }] };
    m.turns.me = { id: 'me:0:0', channel: 'me', order: 0, text: 'Welcome.', start: 1, end: 2, speaker: 'me', final_text: true, words: [] };
    const rows = transcriptRows(m); expect(rows.map(r => r.text)).toEqual(['Welcome.', 'Hello.', 'Hi!']);
    expect(speakerName(m, rows[2].speaker)).toBe('Nicole'); expect(speakerName(m, rows[0].speaker)).toBe('ME');
  });
  test('reconnected A does not inherit the earlier A’s confirmed name', () => {
    const m = meeting(); m.names['remote:0:A'] = 'Travis';
    expect(speakerName(m, 'remote:1:A')).toBe('Speaker A · connection 2');
    expect(speakerName(m, 'remote:0:PENDING')).toBe('Identifying speaker…');
  });
  test('partial shows the current text without splitting unfinished word labels', () => {
    const m = meeting(); m.turns.a = { id: 'a', channel: 'remote', order: 0, text: 'Let us begin', start: 0, end: 1, speaker: 'remote:0:PENDING', final_text: false, words: [{ text: 'Let', start: 0, end: 1, speaker: 'remote:0:A' }] };
    expect(transcriptRows(m)).toHaveLength(1); expect(transcriptRows(m)[0].text).toBe('Let us begin');
  });
});
