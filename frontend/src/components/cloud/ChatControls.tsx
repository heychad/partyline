import { stamp } from './types';

export type ChatSource = { meeting_id: string; title: string; start: number; end: number; text: string };

export function ChatCitations({ sources, indices, open }: {
  sources: ChatSource[];
  indices: number[];
  open: (source: ChatSource) => void;
}) {
  return <div className="cl-citations">{indices.map(index => {
    const source = sources[index - 1];
    return source ? <button className="cl-text-button" key={index} onClick={() => open(source)}>
      [{index}] {source.title} · {stamp(source.start)}
    </button> : null;
  })}</div>;
}

export function ChatComposer({ question, setQuestion, scope, setScope, currentAvailable, busy, error, submit }: {
  question: string; setQuestion: (value: string) => void;
  scope: string; setScope: (value: string) => void;
  currentAvailable: boolean; busy: boolean; error: string; submit: () => void;
}) {
  const disabled = busy || !question.trim();
  return <form onSubmit={event => { event.preventDefault(); if (!disabled) submit(); }}>
    <label>Search<select value={scope} onChange={event => setScope(event.target.value)}>
      <option value="all">All recordings</option>
      {currentAvailable && <option value="current">This recording</option>}
    </select></label>
    <label>Your question<textarea autoFocus rows={2} maxLength={2000} value={question}
      onChange={event => setQuestion(event.target.value)}
      onKeyDown={event => {
        if (event.key !== 'Enter' || event.shiftKey || event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229) return;
        event.preventDefault();
        if (!disabled && !event.repeat) event.currentTarget.form?.requestSubmit();
      }}
      aria-describedby="cl-chat-input-help" placeholder="What did we decide about…?" /></label>
    <p id="cl-chat-input-help" className="cl-field-hint">Enter to send · Shift+Enter for a new line</p>
    <p className="cl-field-hint">Relevant transcript excerpts go to OpenAI. Redacted passages and Trash are excluded.</p>
    {error && <p className="cl-error" role="alert">{error}</p>}
    <div className="cl-actions"><button type="submit" className="cl-button cl-primary" disabled={disabled}>Ask</button></div>
  </form>;
}
