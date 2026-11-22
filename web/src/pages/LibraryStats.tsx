import { Card, Skeleton } from '../design';
import { useMyStats } from '../api/hooks';

/** Humanize a millisecond duration the way `fg stats` does. */
function humanMs(ms: number): string {
  if (ms < 1000) return `${ms} ms`;
  const secs = ms / 1000;
  if (secs < 60) return `${secs.toFixed(1)} s`;
  const mins = secs / 60;
  if (mins < 60) return `${mins.toFixed(1)} min`;
  const hours = mins / 60;
  if (hours < 48) return `${hours.toFixed(1)} h`;
  return `${(hours / 24).toFixed(1)} days`;
}

/**
 * "Your library" summary (Peak C8): aggregates of the
 * caller's OWN cloud runs, reported by the CLI as runs
 * complete. Local runs are never reported - they stay on the
 * machine, where `fg stats` covers everything.
 */
export default function LibraryStats() {
  const stats = useMyStats();
  if (stats.isPending) {
    return (
      <Card>
        <div style={{ padding: 'var(--fh-space-4)' }}>
          <Skeleton lines={2} />
        </div>
      </Card>
    );
  }
  if (stats.isError || !stats.data) {
    return null;
  }
  const data = stats.data;
  return (
    <Card>
      <div style={{ display: 'grid', gap: 'var(--fh-space-3)', padding: 'var(--fh-space-4)' }}>
        <h2 style={{ margin: 0 }}>Your library</h2>
        {data.runs === 0 ? (
          <p style={{ margin: 0 }}>
            No cloud runs yet. When you run a cloud snippet with the CLI, it is counted here.
            Local runs never leave your machine - <code>fg stats</code> covers those.
          </p>
        ) : (
          <>
            <div
              style={{
                display: 'flex',
                gap: 'var(--fh-space-5)',
                flexWrap: 'wrap',
              }}
            >
              <div>
                <div style={{ fontSize: 'var(--md-sys-typescale-headline-medium-size)' }}>{data.runs}</div>
                <div>cloud runs</div>
              </div>
              <div>
                <div style={{ fontSize: 'var(--md-sys-typescale-headline-medium-size)' }}>{data.commands}</div>
                <div>commands automated</div>
              </div>
              <div>
                <div style={{ fontSize: 'var(--md-sys-typescale-headline-medium-size)' }}>{humanMs(data.estimated_saved_ms)}</div>
                <div>estimated time saved</div>
              </div>
              <div>
                <div style={{ fontSize: 'var(--md-sys-typescale-headline-medium-size)' }}>{humanMs(data.total_ms)}</div>
                <div>spent running</div>
              </div>
              {data.failures > 0 ? (
                <div>
                  <div style={{ fontSize: 'var(--md-sys-typescale-headline-medium-size)' }}>{data.failures}</div>
                  <div>failed runs</div>
                </div>
              ) : null}
            </div>
            <div style={{ display: 'grid', gap: 'var(--fh-space-1)' }}>
              {data.workflows.slice(0, 5).map((w) => (
                <div
                  key={w.ref}
                  style={{ display: 'flex', gap: 'var(--fh-space-2)', flexWrap: 'wrap' }}
                >
                  <code style={{ flex: 1, minWidth: 160 }}>{w.ref}</code>
                  <span>
                    {w.runs} run{w.runs === 1 ? '' : 's'}
                    {w.failures > 0 ? ` - ${w.failures} failed` : ''} - last run{' '}
                    {w.last_run.slice(0, 10)}
                  </span>
                </div>
              ))}
            </div>
            <p style={{ margin: 0 }}>
              Counts only your own runs, reported by your CLI. Time saved uses the same
              estimate as <code>fg stats</code>: commands times your recall-and-typing
              allowance, minus the time actually spent running.
            </p>
          </>
        )}
      </div>
    </Card>
  );
}
