import { Link, useNavigate } from 'react-router-dom';
import {
  Button,
  Card,
  CodeBlock,
  CopyIconButton,
  Icon,
  ListItem,
  PageHeader,
  Skeleton,
  StatusChip,
  useToast,
} from '../design';
import { useExplore, useMe, useMySnippets } from '../api/hooks';
import { relativeTime } from '../validation';

/**
 * The signed-in Home: a command centre, deliberately NOT
 * the public landing page (user-directed, 2026-11-20 - the
 * nav's Home used to bounce people out to the marketing
 * scene). Home answers "where was I and what's worth
 * running"; the Library stays the full searchable list and
 * the landing stays the public face of the product.
 * Peak A5: quick actions up top, recent snippets and
 * activity as M3 lists, and a two-step empty state for new
 * accounts.
 */
export default function HomePage() {
  const me = useMe();
  const navigate = useNavigate();
  const toast = useToast();
  const mine = useMySnippets('all', 'updated');
  const popular = useExplore('popular');
  const name = me.data?.display_name || me.data?.username || '';

  const recent = (mine.data?.snippets ?? []).slice(0, 5);
  const trending = (popular.data?.pages[0]?.snippets ?? []).slice(0, 5);
  const isNewUser = !mine.isPending && recent.length === 0;


  return (
    <div>
      <PageHeader
        title={`Welcome back, ${name}`}
        subtitle="Jump back into your commands, or find something new worth saving."
        actions={
          <Button variant="filled" icon="add" onClick={() => navigate('/new')}>
            New snippet
          </Button>
        }
      />

      {isNewUser ? (
        <Card variant="filled">
          <h2 style={{ marginTop: 0 }}>Two steps to your first saved command</h2>
          <ol style={{ display: 'grid', gap: 'var(--fh-space-4)', paddingInlineStart: 'var(--fh-space-5)' }}>
            <li>
              <strong>Get the CLI.</strong> Install ForgeHub on your machine (the README has the
              one-command install), then sign in from your terminal:
              <div style={{ marginTop: 'var(--fh-space-2)' }}>
                <CodeBlock label="Terminal" code="fg login" />
              </div>
            </li>
            <li>
              <strong>Record a command you keep re-typing.</strong> Everything you run is captured
              until you type <code>end</code>:
              <div style={{ marginTop: 'var(--fh-space-2)' }}>
                <CodeBlock label="Terminal" code={'fg record\n# your commands...\nend'} />
              </div>
            </li>
          </ol>
          <p style={{ marginBottom: 0 }}>
            <Button variant="filled" onClick={() => navigate('/new')}>
              Or create a snippet on the web
            </Button>
          </p>
        </Card>
      ) : (
        <>
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
              ) : (
                <>
                  <md-list class="fh-list-enter">
                    {recent.map((s) => (
                      <ListItem
                        key={s.name}
                        headline={s.name}
                        supportingText={`${s.description || 'No description.'} - v${s.version} - ${relativeTime(s.updated_at)}`}
                        leading={<Icon name={s.summary?.mode === 'steps' ? 'library_books' : 'terminal'} />}
                        trailing={<StatusChip visibility={s.visibility} />}
                        onClick={() => navigate(`/${s.owner}/${s.name}`)}
                      />
                    ))}
                  </md-list>
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
                  <md-list class="fh-list-enter">
                    {trending.map((s) => (
                      <ListItem
                        key={`${s.owner}/${s.name}`}
                        headline={s.name}
                        supportingText={`by ${s.owner} - ${relativeTime(s.updated_at)}`}
                        leading={<Icon name="terminal" />}
                        trailing={
                          <span style={{ display: 'inline-flex', gap: 'var(--fh-space-2)', alignItems: 'center' }}>
                            <span style={{ display: 'inline-flex', alignItems: 'center', gap: 'var(--fh-space-1)' }}>
                              <Icon name="star" /> {s.star_count}
                            </span>
                            <span style={{ display: 'inline-flex', alignItems: 'center', gap: 'var(--fh-space-1)' }}>
                              <Icon name="call_split" /> {s.fork_count}
                            </span>
                          </span>
                        }
                        onClick={() => navigate(`/${s.owner}/${s.name}`)}
                      />
                    ))}
                  </md-list>
                  <p style={{ marginBottom: 0 }}>
                    <Link to="/explore">More on Explore</Link>
                  </p>
                </>
              )}
            </Card>
          </div>

          <div
            style={{
              display: 'grid',
              gap: 'var(--fh-space-4)',
              gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))',
            }}
          >
            <Card variant="outlined">
              <h2 style={{ marginTop: 0 }}>Recent activity</h2>
              <md-list class="fh-list-enter">
                {recent.map((s) => (
                  <ListItem
                    key={s.name}
                    headline={`${s.name} reached v${s.version}`}
                    supportingText={relativeTime(s.updated_at)}
                    leading={<Icon name="history" />}
                    onClick={() => navigate(`/${s.owner}/${s.name}`)}
                  />
                ))}
              </md-list>
            </Card>

            <Card variant="filled">
              <h2 style={{ marginTop: 0 }}>Run from the CLI</h2>
              <p>
                Every snippet you save is one short command away, on any machine signed in to
                ForgeHub - shown in full first, and run only when you say yes.
              </p>
              <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center' }}>
                <code style={{ flex: 1, fontSize: 'var(--md-sys-typescale-body-large-size)' }}>fg ubuntu-setup</code>
                <CopyIconButton text="fg ubuntu-setup" label="Copy run command" onCopied={() => toast({ message: 'Copied: fg ubuntu-setup' })} />
              </div>
            </Card>
          </div>
        </>
      )}
    </div>
  );
}
