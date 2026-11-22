import { Card, Skeleton } from '../design';
import { useMyStats } from '../api/hooks';
import { fmt, m } from '../i18n/messages';

/** Humanize a millisecond duration the way `fg stats` does. */
function humanMs(ms: number): string {
  if (ms < 1000) return fmt(m.libraryStats.unitMs, { value: ms });
  const secs = ms / 1000;
  if (secs < 60) return fmt(m.libraryStats.unitSeconds, { value: secs.toFixed(1) });
  const mins = secs / 60;
  if (mins < 60) return fmt(m.libraryStats.unitMinutes, { value: mins.toFixed(1) });
  const hours = mins / 60;
  if (hours < 48) return fmt(m.libraryStats.unitHours, { value: hours.toFixed(1) });
  return fmt(m.libraryStats.unitDays, { value: (hours / 24).toFixed(1) });
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
        <h2 style={{ margin: 0 }}>{m.libraryStats.title}</h2>
        {data.runs === 0 ? (
          <p style={{ margin: 0 }}>
            {m.libraryStats.emptyBefore} <code>fg stats</code> {m.libraryStats.emptyAfter}
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
                <div>{m.libraryStats.cloudRuns}</div>
              </div>
              <div>
                <div style={{ fontSize: 'var(--md-sys-typescale-headline-medium-size)' }}>{data.commands}</div>
                <div>{m.libraryStats.commandsAutomated}</div>
              </div>
              <div>
                <div style={{ fontSize: 'var(--md-sys-typescale-headline-medium-size)' }}>{humanMs(data.estimated_saved_ms)}</div>
                <div>{m.libraryStats.estimatedSaved}</div>
              </div>
              <div>
                <div style={{ fontSize: 'var(--md-sys-typescale-headline-medium-size)' }}>{humanMs(data.total_ms)}</div>
                <div>{m.libraryStats.spentRunning}</div>
              </div>
              {data.failures > 0 ? (
                <div>
                  <div style={{ fontSize: 'var(--md-sys-typescale-headline-medium-size)' }}>{data.failures}</div>
                  <div>{m.libraryStats.failedRuns}</div>
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
                    {fmt(m.libraryStats.runsLine, {
                      runs: w.runs,
                      runWord: w.runs === 1 ? m.libraryStats.runOne : m.libraryStats.runMany,
                      failedPart:
                        w.failures > 0 ? fmt(m.libraryStats.failedPart, { count: w.failures }) : '',
                      date: w.last_run.slice(0, 10),
                    })}
                  </span>
                </div>
              ))}
            </div>
            <p style={{ margin: 0 }}>
              {m.libraryStats.footnoteBefore} <code>fg stats</code>{m.libraryStats.footnoteAfter}
            </p>
          </>
        )}
      </div>
    </Card>
  );
}
