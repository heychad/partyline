import { Component, createRef, type ReactNode } from 'react';

type Snapshot = { top: number; id?: string; offset: number } | null;
// React's pre-commit snapshot captures the old DOM before streamed notes replace
// it. Keep the same section at the same screen offset, even if content above grew.
export default class NotesScroll extends Component<{ version: string; children: ReactNode }> {
  viewport = createRef<HTMLDivElement>();
  getSnapshotBeforeUpdate(previous: { version: string }): Snapshot {
    const viewport = this.viewport.current;
    if (!viewport || previous.version === this.props.version) return null;
    const top = viewport.getBoundingClientRect().top;
    const anchor = Array.from(viewport.querySelectorAll<HTMLElement>('[data-outline-id]')).find(el => el.getBoundingClientRect().bottom > top && el.getBoundingClientRect().top >= top - 60);
    return { top: viewport.scrollTop, id: anchor?.dataset.outlineId, offset: anchor ? anchor.getBoundingClientRect().top - top : 0 };
  }
  componentDidUpdate(_previous: unknown, _state: unknown, snapshot: Snapshot) {
    const viewport = this.viewport.current;
    if (!viewport || !snapshot) return;
    const anchor = Array.from(viewport.querySelectorAll<HTMLElement>('[data-outline-id]')).find(el => el.dataset.outlineId === snapshot.id);
    viewport.scrollTop = anchor ? viewport.scrollTop + anchor.getBoundingClientRect().top - viewport.getBoundingClientRect().top - snapshot.offset : snapshot.top;
  }
  render() { return <div className="cl-scroll" ref={this.viewport}>{this.props.children}</div>; }
}
