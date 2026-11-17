// DOM adapter is injected so the same behavior is exercised without a window.
export class TranscriptFollower {
  enabled = true;
  private lastTop = 0;
  private element: { scrollTop: number; scrollHeight: number; clientHeight: number };
  private changed: (enabled: boolean) => void;
  constructor(element: { scrollTop: number; scrollHeight: number; clientHeight: number }, changed: (enabled: boolean) => void) { this.element = element; this.changed = changed; }
  set(enabled: boolean) { this.enabled = enabled; if (enabled) this.contentChanged(); }
  contentChanged() {
    if (this.enabled) this.element.scrollTop = Math.max(0, this.element.scrollHeight - this.element.clientHeight);
    this.lastTop = this.element.scrollTop;
  }
  upwardIntent() { if (this.enabled) { this.enabled = false; this.changed(false); } }
  scrolled() {
    // Only upward movement away from the bottom pauses following. Growing
    // content and downward wheel events must not disable it.
    const { scrollTop, scrollHeight, clientHeight } = this.element;
    if (scrollTop < this.lastTop - 2 && scrollHeight - clientHeight - scrollTop > 8) this.upwardIntent();
    this.lastTop = scrollTop;
  }
}
