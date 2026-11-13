/** Style existing source labels without changing transcript or clipboard text. */
export function SpeakerText({ text }: { text: string }) {
  const match = /^(ME|THEM):/.exec(text);
  if (!match) return <>{text}</>;
  return <><span className={match[1] === 'ME' ? 'font-semibold text-speaker-me' : 'font-semibold text-speaker-them'}>{match[0]}</span>{text.slice(match[0].length)}</>;
}
