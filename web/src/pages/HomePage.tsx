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
import { fmt, m } from '../i18n/messages';

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
        title={fmt(m.home.welcome, { name })}
        subtitle={m.home.subtitle}
        actions={
          <Button variant="filled" icon="add" onClick={() => navigate('/new')}>
            {m.home.newSnippet}
          </Button>
        }
      />

      {isNewUser ? (
        <Card variant="filled">
          <h2 style={{ marginTop: 0 }}>{m.home.firstStepsTitle}</h2>
          <ol style={{ display: 'grid', gap: 'var(--fh-space-4)', paddingInlineStart: 'var(--fh-space-5)' }}>
            <li>
              <strong>{m.home.step1Strong}</strong> {m.home.step1Rest}
              <div style={{ marginTop: 'var(--fh-space-2)' }}>
                <CodeBlock label={m.home.terminal} code="fg login" />
              </div>
            </li>
            <li>
              <strong>{m.home.step2Strong}</strong> {m.home.step2RestBefore} <code>end</code>:
              <div style={{ marginTop: 'var(--fh-space-2)' }}>
                <CodeBlock label={m.home.terminal} code={'fg record\n# your commands...\nend'} />
              </div>
            </li>
          </ol>
          <p style={{ marginBottom: 0 }}>
            <Button variant="filled" onClick={() => navigate('/new')}>
              {m.home.createOnWeb}
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
              <h2 style={{ marginTop: 0 }}>{m.home.recentTitle}</h2>
              {mine.isPending ? (
                <Skeleton lines={3} />
              ) : (
                <>
                  <md-list class="fh-list-enter">
                    {recent.map((s) => (
                      <ListItem
                        key={s.name}
                        headline={s.name}
                        supportingText={fmt(m.home.recentLine, { description: s.description || m.home.noDescription, version: s.version, when: relativeTime(s.updated_at) })}
                        leading={<Icon name={s.summary?.mode === 'steps' ? 'library_books' : 'terminal'} />}
                        trailing={<StatusChip visibility={s.visibility} />}
                        onClick={() => navigate(`/${s.owner}/${s.name}`)}
                      />
                    ))}
                  </md-list>
                  <p style={{ marginBottom: 0 }}>
                    <Link to="/dashboard">{m.home.viewAllLibrary}</Link>
                  </p>
                </>
              )}
            </Card>

            <Card variant="outlined">
              <h2 style={{ marginTop: 0 }}>{m.home.popularTitle}</h2>
              {popular.isPending ? (
                <Skeleton lines={3} />
              ) : trending.length === 0 ? (
                <p>{m.home.popularEmpty}</p>
              ) : (
                <>
                  <md-list class="fh-list-enter">
                    {trending.map((s) => (
                      <ListItem
                        key={`${s.owner}/${s.name}`}
                        headline={s.name}
                        supportingText={fmt(m.home.byLine, { owner: s.owner, when: relativeTime(s.updated_at) })}
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
                    <Link to="/explore">{m.home.moreExplore}</Link>
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
              <h2 style={{ marginTop: 0 }}>{m.home.activityTitle}</h2>
              <md-list class="fh-list-enter">
                {recent.map((s) => (
                  <ListItem
                    key={s.name}
                    headline={fmt(m.home.reachedVersion, { name: s.name, version: s.version })}
                    supportingText={relativeTime(s.updated_at)}
                    leading={<Icon name="history" />}
                    onClick={() => navigate(`/${s.owner}/${s.name}`)}
                  />
                ))}
              </md-list>
            </Card>

            <Card variant="filled">
              <h2 style={{ marginTop: 0 }}>{m.home.runFromCliTitle}</h2>
              <p>
                {m.home.runFromCliBody}
              </p>
              <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center' }}>
                <code style={{ flex: 1, fontSize: 'var(--md-sys-typescale-body-large-size)' }}>fg ubuntu-setup</code>
                <CopyIconButton text="fg ubuntu-setup" label={m.home.copyRunCommand} onCopied={() => toast({ message: m.home.copiedRunCommand })} />
              </div>
            </Card>
          </div>
        </>
      )}
    </div>
  );
}
