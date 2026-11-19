import { Link, useNavigate, useParams } from 'react-router-dom';
import { Card, EmptyState, Icon, Skeleton, StatusChip } from '../design';
import { useProfile, useProfileSnippets } from '../api/hooks';
import { relativeTime } from '../validation';

/**
 * A public profile: display name, bio, join date and public
 * snippets. The API never returns an email here, and neither
 * does this page.
 */
export default function ProfilePage() {
  const { username = '' } = useParams<{ username: string }>();
  const profile = useProfile(username);
  const snippets = useProfileSnippets(username);
  const navigate = useNavigate();

  if (profile.isPending) {
    return (
      <Card variant="outlined">
        <Skeleton lines={4} />
      </Card>
    );
  }
  if (profile.isError || !profile.data) {
    return (
      <EmptyState
        icon="person"
        title="No such user"
        message="That profile does not exist."
        actionLabel="Go to Explore"
        onAction={() => navigate('/explore')}
      />
    );
  }
  const p = profile.data;
  return (
    <div style={{ display: 'grid', gap: 'var(--fh-space-4)' }}>
      <div style={{ display: 'flex', gap: 'var(--fh-space-3)', alignItems: 'flex-start' }}>
        <span
          aria-hidden="true"
          style={{
            width: 56,
            height: 56,
            borderRadius: 'var(--md-sys-shape-corner-full)',
            background: 'var(--md-sys-color-surface-container-highest)',
            display: 'inline-flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontSize: 'var(--md-sys-typescale-headline-small-size)',
            fontWeight: 500,
          }}
        >
          {p.username.charAt(0).toUpperCase()}
        </span>
        <div>
          <h1 style={{ margin: 0, fontSize: 'var(--md-sys-typescale-headline-medium-size)', fontWeight: 500 }}>
            {p.display_name || p.username}
          </h1>
          <p style={{ margin: 'var(--fh-space-1) 0', color: 'var(--md-sys-color-on-surface-variant)' }}>
            @{p.username} · joined{' '}
            {new Date(p.created_at).toLocaleDateString(undefined, { year: 'numeric', month: 'long' })} ·{' '}
            {p.public_snippet_count} public snippet{p.public_snippet_count === 1 ? '' : 's'}
          </p>
          {p.bio ? <p style={{ margin: 0 }}>{p.bio}</p> : null}
        </div>
      </div>

      <h2 style={{ margin: 0 }}>Public snippets</h2>
      {snippets.isPending ? (
        <Skeleton lines={3} />
      ) : snippets.data && snippets.data.snippets.length > 0 ? (
        <Card variant="outlined">
          <md-list>
            {snippets.data.snippets.map((s) => (
              <md-list-item key={s.name} type="button" onClick={() => navigate(`/${p.username}/${s.name}`)}>
                <span slot="headline">
                  <Link to={`/${p.username}/${s.name}`} style={{ color: 'inherit' }}>
                    {s.name}
                  </Link>
                </span>
                <span slot="supporting-text">
                  {s.description || 'No description.'} · v{s.version} · {relativeTime(s.updated_at)}
                </span>
                <span slot="start">
                  <Icon name="terminal" />
                </span>
                <span slot="end">
                  <StatusChip visibility={s.visibility} />
                </span>
              </md-list-item>
            ))}
          </md-list>
        </Card>
      ) : (
        <EmptyState icon="library_books" title="Nothing public yet" message="This user has not shared any public snippets." />
      )}
    </div>
  );
}
