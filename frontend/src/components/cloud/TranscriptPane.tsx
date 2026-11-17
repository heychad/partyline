import { useLayoutEffect, useRef } from 'react';
import { ArrowDown, Mic } from 'lucide-react';
import type { Meeting, Row } from './types';
import { speakerName, stamp } from './types';
import { paragraphSources, type SpeakerGroup } from './transcript';
import { TranscriptFollower } from './follow';

export default function TranscriptPane({ meeting, groups, active, independent, follow, setFollow, play, edit, redact }: {
  meeting: Meeting; groups: SpeakerGroup[]; active: boolean; independent: boolean; follow: boolean;
  setFollow: (enabled: boolean) => void; play: (position: number) => void;
  edit: (row: Row) => void; redact: (row: Row) => void;
}) {
  const viewport = useRef<HTMLDivElement>(null), content = useRef<HTMLDivElement>(null);
  const follower = useRef<TranscriptFollower>();
  const callback = useRef(setFollow); callback.current = setFollow;
  const touchY = useRef<number>();
  useLayoutEffect(() => {
    if (!viewport.current || !content.current) return;
    const scroller = independent ? viewport.current : viewport.current.closest<HTMLElement>('.cl-main') ?? viewport.current;
    const controller = new TranscriptFollower(scroller, value => callback.current(value));
    const onScroll = () => controller.scrolled();
    scroller.addEventListener('scroll', onScroll);
    follower.current = controller;
    controller.set(follow);
    let frame = 0;
    const observer = new ResizeObserver(() => { cancelAnimationFrame(frame); frame = requestAnimationFrame(() => controller.contentChanged()); });
    observer.observe(content.current); observer.observe(scroller);
    return () => { observer.disconnect(); scroller.removeEventListener('scroll', onScroll); cancelAnimationFrame(frame); follower.current = undefined; };
  }, [meeting.id, independent]);
  useLayoutEffect(() => { follower.current?.set(follow); }, [follow, groups]);
  const unavailable = (row: Row) => active || !!meeting.deleted_at || row.start >= (meeting.trim_end ?? meeting.duration) || row.end <= (meeting.trim_start ?? 0);
  return <section className="cl-panel cl-transcript-panel" aria-label="Transcript">
    <div className="cl-panel-heading"><h2>Transcript</h2>{active && <button className={`cl-follow ${follow ? 'enabled' : ''}`} aria-pressed={follow} onClick={() => setFollow(!follow)}><ArrowDown size={14} />{follow ? 'Following live' : 'Follow live'}</button>}</div>
    <div className="cl-scroll cl-transcript" ref={viewport} tabIndex={0} aria-label="Transcript passages"
      onScroll={() => follower.current?.scrolled()}
      onWheel={e => { if (e.deltaY < 0) follower.current?.upwardIntent(); }}
      onKeyDown={e => { if (['ArrowUp', 'PageUp', 'Home'].includes(e.key)) follower.current?.upwardIntent(); }}
      onTouchStart={e => { touchY.current = e.touches[0]?.clientY; }}
      onTouchMove={e => { const y = e.touches[0]?.clientY; if (y !== undefined && touchY.current !== undefined && y > touchY.current + 2) follower.current?.upwardIntent(); touchY.current = y; }}>
      <div ref={content}>
        {!groups.length && <div className="cl-empty"><Mic size={23} /><p>{active ? 'Words will appear as people speak.' : 'No transcript was captured.'}</p></div>}
        {groups.map(group => <article className="cl-speaker-group" key={group.id}>
          <div className="cl-speaker-heading"><span className={`cl-avatar ${group.speaker === 'me' ? 'cl-me' : 'cl-them'}`}>{speakerName(meeting, group.speaker).split(/\s+/).map(w => w[0]).slice(0, 2).join('')}</span><span className={`cl-speaker ${group.speaker === 'me' ? 'cl-me' : 'cl-them'}`}>{speakerName(meeting, group.speaker)}</span><time>{stamp(group.start)}</time></div>
          {group.paragraphs.map(paragraph => <div className="cl-paragraph" key={paragraph.id}>
            <p>{paragraph.fragments.map((row, i) => <span key={row.id} className={!row.final ? 'cl-partial' : undefined}>{i > 0 ? ' ' : ''}<span className="cl-passage" role="button" tabIndex={unavailable(row) ? -1 : 0} aria-disabled={unavailable(row)} aria-label={`Listen at ${stamp(row.start)}: ${row.text}`} onClick={() => { if (!unavailable(row)) play(Math.max(meeting.trim_start ?? 0, row.start)); }} onKeyDown={e => { if (!unavailable(row) && ['Enter', ' '].includes(e.key)) { e.preventDefault(); play(Math.max(meeting.trim_start ?? 0, row.start)); } }}>{row.text}</span></span>)}</p>
            <div className="cl-passage-actions"><button className="cl-text-button" disabled={unavailable(paragraph.fragments[0])} onClick={() => play(Math.max(meeting.trim_start ?? 0, paragraph.fragments[0].start))}><time>{stamp(paragraph.fragments[0].start)}</time></button>
              {!active && !meeting.deleted_at && <details className="cl-passage-menu"><summary>Edit / redact</summary><div>{paragraphSources(paragraph).map(row => <div key={row.turn_id}><span>{stamp(row.start)}</span><button onClick={() => edit(row)}>Edit text</button><button onClick={() => redact(row)}>Redact passage</button></div>)}</div></details>}
              {paragraph.fragments.some(row => meeting.redactions?.some(r => row.start < r.end && row.end > r.start)) && <span className="cl-redacted">Contains redacted text</span>}
              {paragraph.fragments.some(row => !row.final) && <span className="cl-muted">Live</span>}
            </div>
          </div>)}
        </article>)}
      </div>
    </div>
  </section>;
}
