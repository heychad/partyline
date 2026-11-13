import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { SpeakerText } from '../../src/components/SpeakerText';

describe('speaker labels', () => {
  test('styles source labels while preserving the complete transcript text', () => {
    for (const [label, color] of [['ME', 'me'], ['THEM', 'them']]) {
      const text = `${label}: Keep ME: inside the message.\nSecond line.`;
      const html = renderToStaticMarkup(<SpeakerText text={text} />);
      expect(html).toContain(`text-speaker-${color}`);
      expect(html.replace(/<[^>]+>/g, '')).toBe(text);
      expect(html.match(/<span/g)).toHaveLength(1);
    }
  });

  test('leaves unlabelled and legacy transcripts unchanged', () => {
    for (const text of ['', 'Speaker 1: Hello', 'They told ME: hello', 'MEMO: hello']) {
      expect(renderToStaticMarkup(<SpeakerText text={text} />)).toBe(text);
    }
  });

  test('escapes transcript content as text', () => {
    expect(renderToStaticMarkup(<SpeakerText text={'ME: <script> & THEM:'} />))
      .toContain('&lt;script&gt; &amp; THEM:');
  });
});
