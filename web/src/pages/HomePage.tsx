import { Link, useNavigate } from 'react-router-dom';
import { Button, Card, CodeBlock, Icon, PageHeader, Skeleton } from '../design';
import { useExplore, useMe, useMySnippets } from '../api/hooks';
import type { Snippet } from '../api/types';
import { relativeTime } from '../validation';

/**
 * The signed-in Home: a command centre, deliberately NOT
 * the public landing page (user-directed, 2026-11-20 - the
 * nav's Home used to bounce people out to the marketing
 * scene). Home answers "where was I and what's worth
 * running"; the Library stays the full searchable list and
 * the landing stays the public face of the product.
 */
export default function HomePage() {
  const me = useMe();
  const navigate = useNavigate();
  const mine = useMySnippets('all', 'updated');
  const popular = useExplore('popular');
  const name = me.data?.display_name || me.data?.username || '';

  const recent = (mine.data?.snippets ?? []).slice(0, 5);
  const trending = (popular.data?.pages[0]?.snippets ?? []).slice(0, 5);

  return (
    <div>
      <PageHeader
        title={`Welcome back, ${name}`}
        subtitle="Jump back into your commands, or find something new worth saving."
        actions={
          <Button variant="filled" onClick={() => navigate('/new')}>
            <Icon name="add" /> New snippet
          </Button>
        }
      />

      <div
        style={{
          display: 'grid',
          gap: 'var(--fh-space-4)',
          gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))',
          marginBottom: 'var(--fh-space-4)',
        }}
      >
        <Card variant="outlined">
          <h2 style={{ marginTop: 0 }}>Your recent snippets</h2>
          {mine.isPending ? (
            <Skeleton lines={3} />
          ) : recent.length === 0 ? (
            <>
              <p>Nothing saved yet. Your first command is the hardest one to re-type from memory - save it once.</p>
              <Button variant="tonal" onClick={() => navigate('/new')}>
                Save your first snippet
              </Button>
            </>
          ) : (
            <>
              <div style={{ display: 'grid', gap: 'var(--fh-space-3)' }}>
                {recent.map((s) => (
                  <RecentRow key={s.name} snippet={s} />
                ))}
              </div>
              <p style={{ marginBottom: 0 }}>
                <Link to="/dashboard">View all in your Library</Link>
              </p>
            </>
          )}
        </Card>

        <Card variant="outlined">
          <h2 style={{ marginTop: 0 }}>Popular this week</h2>
          {popular.isPending ? (
            <Skeleton lines={3} />
          ) : trending.length === 0 ? (
            <p>No public snippets yet - yours could be the first everyone reaches for.</p>
          ) : (
            <>
              <div style={{ display: 'grid', gap: 'var(--fh-space-3)' }}>
                {trending.map((s) => (
                  <PopularRow key={`${s.owner}/${s.name}`} snippet={s} />
                ))}
              </div>
              <p style={{ marginBottom: 0 }}>
                <Link to="/explore">More on Explore</Link>
              </p>
            </>
          )}
        </Card>
      </div>

      <Card variant="filled">
        <h2 style={{ marginTop: 0 }}>Run it anywhere</h2>
        <p>
          Every snippet you save is one short command away, on any machine signed in to ForgeHub - shown in full
          first, and run only when you say yes.
        </p>
        <CodeBlock label="Terminal" code="$ fg ubuntu-setup" />
      </Card>
    </div>
  );
}

function RecentRow({ snippet }: { snippet: Snippet }) {
  return (
    <div>
      <Link to={`/${snippet.owner}/${snippet.name}`} style={{ color: 'inherit', fontWeight: 600 }}>
        {snippet.name}
      </Link>{' '}
      <span style={{ color: 'var(--md-sys-color-on-surface-variant)' }}>
        · v{snippet.version} · {relativeTime(snippet.updated_at)}
      </span>
      <div style={{ color: 'var(--md-sys-color-on-surface-variant)' }}>
        {snippet.description || 'No description.'}
      </div>
    </div>
  );
}

function PopularRow({ snippet }: { snippet: Snippet }) {
  return (
    <div>
      <Link to={`/${snippet.owner}/${snippet.name}`} style={{ color: 'inherit', fontWeight: 600 }}>
        {snippet.name}
      </Link>{' '}
      <span style={{ color: 'var(--md-sys-color-on-surface-variant)' }}>by {snippet.owner}</span>
      <div style={{ display: 'flex', gap: 'var(--fh-space-3)', color: 'var(--md-sys-color-on-surface-variant)' }}>
        <span style={{ display: 'inline-flex', alignItems: 'center', gap: 'var(--fh-space-1)' }}>
          <Icon name="star" /> {snippet.star_count}
        </span>
        <span style={{ display: 'inline-flex', alignItems: 'center', gap: 'var(--fh-space-1)' }}>
          <Icon name="call_split" /> {snippet.fork_count}
        </span>
        <span>{relativeTime(snippet.updated_at)}</span>
      </div>
    </div>
  );
}
