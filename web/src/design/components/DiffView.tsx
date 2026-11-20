/**
 * Unified diff renderer. Additions and deletions are told
 * apart by the +/- gutter symbol AND a tonal background shift
 * (design-spec §1) - never by colour alone, and never
 * red/green. All diff text renders as text.
 */
export function DiffView({ diff }: { diff: string }) {
  const lines = diff.replace(/\n$/, '').split('\n');
  return (
    <div className="fh-diff" role="table" aria-label="Diff">
      {lines.map((line, i) => {
        const kind = line.startsWith('+') && !line.startsWith('+++')
          ? 'add'
          : line.startsWith('-') && !line.startsWith('---')
            ? 'remove'
            : line.startsWith('@@')
              ? 'hunk'
              : 'context';
        const gutter = kind === 'add' ? '+' : kind === 'remove' ? '−' : '';
        const text = kind === 'add' || kind === 'remove' ? line.slice(1) : line;
        return (
          <div className={`fh-diff__row fh-diff__row--${kind}`} key={i} role="row">
            <span className="fh-diff__gutter" aria-hidden="true">
              {gutter}
            </span>
            <code className="fh-diff__text">{text === '' ? ' ' : text}</code>
          </div>
        );
      })}
    </div>
  );
}
