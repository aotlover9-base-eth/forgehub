import { useMemo, useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import {
  Button,
  Card,
  Checkbox,
  Chip,
  CodeBlock,
  DialogActions,
  EmptyState,
  ErrorState,
  Icon,
  IconButton,
  ListItem,
  Menu,
  Skeleton,
  StatusChip,
  Tabs,
  TextField,
  useToast,
} from '../design';
import { DiffView } from '../design/components/DiffView';
import { apiUrl } from '../api/client';
import {
  useDeleteSnippet,
  useDiff,
  useFork,
  useMe,
  useReport,
  useSetVisibility,
  useSnippet,
  useSnippetVersion,
  useStar,
  useForks,
  useVersions,
} from '../api/hooks';
import type { Visibility } from '../api/types';
import { relativeTime } from '../validation';

const VISIBILITIES: Visibility[] = ['private', 'unlisted', 'public'];

export default function SnippetPage() {
  const { owner = '', name = '' } = useParams<{ owner: string; name: string }>();
  const snippet = useSnippet(owner, name);
  const me = useMe();
  const navigate = useNavigate();
  const toast = useToast();
  const [tab, setTab] = useState(0);
  const [viewVersion, setViewVersion] = useState<number | null>(null);
  const [selected, setSelected] = useState<number[]>([]);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleteName, setDeleteName] = useState('');
  const [reportOpen, setReportOpen] = useState(false);
  const [reportReason, setReportReason] = useState('');
  const [forkOpen, setForkOpen] = useState(false);
  const [forkName, setForkName] = useState(name);

  const own = me.data?.username === owner;
  const versionView = useSnippetVersion(owner, name, viewVersion);
  const versions = useVersions(owner, name, tab === 1);
  const forks = useForks(owner, name, tab === 2);
  const [diffPair, setDiffPair] = useState<{ from: number; to: number } | null>(null);
  const diff = useDiff(owner, name, diffPair?.from ?? null, diffPair?.to ?? null);

  const star = useStar();
  const fork = useFork();
  const report = useReport();
  const deleteSnippet = useDeleteSnippet();
  const setVisibility = useSetVisibility();

  const shown = viewVersion !== null && versionView.data ? versionView.data : snippet.data;
  const runCommand = useMemo(() => {
    if (!snippet.data) return '';
    return own ? `fg ${name}` : `fg ${owner}/${name}`;
  }, [snippet.data, own, owner, name]);

  if (snippet.isPending) {
    return (
      <Card variant="outlined">
        <Skeleton lines={6} />
      </Card>
    );
  }
  if (snippet.isError || !snippet.data) {
    // Private-to-others and missing look identical (no reveal).
    return (
      <EmptyState
        icon="search"
        title="Not found"
        message="This snippet does not exist, or it is private."
        actionLabel="Go to Explore"
        onAction={() => navigate('/explore')}
      />
    );
  }
  const s = snippet.data;

  const copy = async (text: string, message: string) => {
    try {
      await navigator.clipboard.writeText(text);
      toast({ message });
    } catch {
      toast({ message: 'Copy failed - your browser blocked clipboard access' });
    }
  };

  const toggleVersion = (version: number) => {
    setSelected((prev) => {
      const next = prev.includes(version) ? prev.filter((v) => v !== version) : [...prev, version];
      return next.slice(-2);
    });
  };

  return (
    <div style={{ display: 'grid', gap: 'var(--fh-space-4)' }}>
      <div style={{ display: 'flex', gap: 'var(--fh-space-3)', alignItems: 'flex-start', flexWrap: 'wrap' }}>
        <span
          aria-hidden="true"
          style={{
            width: 48,
            height: 48,
            borderRadius: 'var(--md-sys-shape-corner-full)',
            background: 'var(--md-sys-color-surface-container-highest)',
            display: 'inline-flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontSize: 'var(--md-sys-typescale-title-large-size)',
            fontWeight: 500,
          }}
        >
          {owner.charAt(0).toUpperCase()}
        </span>
        <div style={{ flex: 1, minWidth: 240 }}>
          <h1 style={{ margin: 0, fontSize: 'var(--md-sys-typescale-headline-medium-size)', fontWeight: 500 }}>
            <Link to={`/${owner}`} style={{ color: 'inherit' }}>
              {owner}
            </Link>
            {' / '}
            {name}
          </h1>
          <p style={{ margin: 'var(--fh-space-1) 0', color: 'var(--md-sys-color-on-surface-variant)' }}>
            {s.description || 'No description.'}
          </p>
          <p style={{ display: 'flex', gap: 'var(--fh-space-3)', alignItems: 'center', flexWrap: 'wrap', margin: 0 }}>
            <StatusChip visibility={s.visibility} />
            <span>
              <Icon name="star" filled={s.starred_by_me} /> {s.star_count}
            </span>
            <span>
              <Icon name="call_split" /> {s.fork_count} forks
            </span>
            <span>
              <Icon name="download" /> {s.pull_count} pulls
            </span>
            <span title={new Date(s.updated_at).toLocaleString()}>v{s.version} · updated {relativeTime(s.updated_at)}</span>
          </p>
          {s.forked_from ? (
            <p style={{ color: 'var(--md-sys-color-on-surface-variant)' }}>
              Forked from{' '}
              <Link to={`/${s.forked_from.owner}/${s.forked_from.name}`}>
                {s.forked_from.owner}/{s.forked_from.name}
              </Link>{' '}
              v{s.forked_from.version}
            </p>
          ) : null}
          {s.tags.length > 0 ? (
            <p style={{ display: 'flex', gap: 'var(--fh-space-1)', flexWrap: 'wrap' }}>
              {s.tags.map((tag) => (
                <Chip key={tag} label={tag} />
              ))}
            </p>
          ) : null}
        </div>
        <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center' }}>
          {own ? (
            <>
              <Button variant="tonal" icon="edit" onClick={() => navigate(`/${owner}/${name}/edit`)}>
                Edit
              </Button>
              <Menu
                buttonLabel="Snippet actions"
                items={[
                  ...VISIBILITIES.filter((v) => v !== s.visibility).map((v) => ({
                    id: `visibility:${v}`,
                    label: `Make ${v}`,
                    icon: v === 'public' ? 'public' : v === 'unlisted' ? 'link' : 'lock',
                  })),
                  { id: 'delete', label: 'Delete', icon: 'delete' },
                ]}
                onSelect={(id) => {
                  if (id === 'delete') {
                    setDeleteName('');
                    setDeleteOpen(true);
                  } else if (id.startsWith('visibility:')) {
                    const visibility = id.slice('visibility:'.length) as Visibility;
                    setVisibility.mutate(
                      { owner, name, visibility },
                      {
                        onSuccess: () => toast({ message: `${name} is now ${visibility}` }),
                        onError: (e) => toast({ message: e.uiMessage }),
                      },
                    );
                  }
                }}
              />
            </>
          ) : me.data ? (
            <>
              <Button
                variant={s.starred_by_me ? 'tonal' : 'outlined'}
                icon="star"
                onClick={() =>
                  star.mutate(
                    { owner, name, starred: s.starred_by_me },
                    { onError: (e) => toast({ message: e.uiMessage }) },
                  )
                }
              >
                {s.starred_by_me ? 'Starred' : 'Star'}
              </Button>
              <Button
                variant="outlined"
                icon="call_split"
                onClick={() => {
                  setForkName(name);
                  setForkOpen(true);
                }}
              >
                Fork
              </Button>
              <IconButton icon="report" label="Report this snippet" onClick={() => setReportOpen(true)} />
            </>
          ) : null}
          <a href={apiUrl(`/snippets/${owner}/${name}/raw`)} target="_blank" rel="noreferrer">
            <Button variant="text">Raw</Button>
          </a>
        </div>
      </div>

      {!own && s.visibility === 'public' ? (
        <Card variant="filled">
          <p style={{ margin: 0, display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center' }}>
            <Icon name="shield" />
            Review commands before you run them. <code>fg</code> will show you exactly what will run
            and ask you to confirm.
          </p>
        </Card>
      ) : null}

      <Card variant="outlined">
        <h2 style={{ marginTop: 0 }}>Run it</h2>
        <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center' }}>
          <code style={{ flex: 1, fontSize: 'var(--md-sys-typescale-body-large-size)' }}>{runCommand}</code>
          <IconButton icon="content_copy" label="Copy run command" onClick={() => void copy(runCommand, `Copied: ${runCommand}`)} />
        </div>
        <p style={{ color: 'var(--md-sys-color-on-surface-variant)', marginBottom: 0 }}>
          New to <code>fg</code>? Install instructions are in the ForgeHub README.
        </p>
      </Card>

      <Tabs
        ariaLabel="Snippet sections"
        tabs={[
          { id: 'code', label: 'Code' },
          { id: 'versions', label: 'Versions' },
          { id: 'forks', label: 'Forks' },
        ]}
        activeIndex={tab}
        onChange={setTab}
      />

      {tab === 0 ? (
        <div style={{ display: 'grid', gap: 'var(--fh-space-3)' }}>
          {viewVersion !== null ? (
            <p>
              Viewing version {viewVersion}.{' '}
              <Button variant="text" onClick={() => setViewVersion(null)}>
                Back to latest (v{s.version})
              </Button>
            </p>
          ) : null}
          {shown?.summary && shown.summary.lint_warnings.length > 0 ? (
            <Card variant="filled">
              <p style={{ margin: 0, fontWeight: 500 }}>Warnings</p>
              <ul>
                {shown.summary.lint_warnings.map((w, i) => (
                  <li key={i}>
                    Line {w.line}: {w.message}
                  </li>
                ))}
              </ul>
              {shown.summary.uses_sudo ? <p style={{ margin: 0 }}>This snippet uses sudo.</p> : null}
            </Card>
          ) : shown?.summary?.uses_sudo ? (
            <p>
              <Icon name="warning" /> This snippet uses sudo.
            </p>
          ) : null}
          <CodeBlock code={shown?.body ?? ''} lineNumbers label={`${name} · v${shown?.version ?? s.version}`} />
        </div>
      ) : tab === 1 ? (
        <div style={{ display: 'grid', gap: 'var(--fh-space-3)' }}>
          {versions.isPending ? (
            <Skeleton lines={3} />
          ) : versions.isError ? (
            <ErrorState title="Could not load versions" message="Try again." retryLabel="Retry" onRetry={() => void versions.refetch()} />
          ) : (
            <Card variant="outlined">
              <md-list>
                {(versions.data?.versions ?? []).map((v) => (
                  <div key={v.version} style={{ display: 'flex', alignItems: 'center', gap: 'var(--fh-space-2)' }}>
                    <Checkbox
                      ariaLabel={`Select version ${v.version} for diff`}
                      checked={selected.includes(v.version)}
                      onChange={() => toggleVersion(v.version)}
                    />
                    <div style={{ flex: 1 }}>
                      <md-list-item>
                        <span slot="headline">
                          v{v.version} · {v.short_hash}
                        </span>
                        <span slot="supporting-text">
                          {v.message || 'No message'} ·{' '}
                          <span title={new Date(v.created_at).toLocaleString()}>{relativeTime(v.created_at)}</span>
                        </span>
                      </md-list-item>
                    </div>
                    <Button
                      variant="text"
                      onClick={() => {
                        setViewVersion(v.version);
                        setTab(0);
                      }}
                    >
                      View code
                    </Button>
                  </div>
                ))}
              </md-list>
            </Card>
          )}
          <p style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center' }}>
            <Button
              variant="tonal"
              disabled={selected.length !== 2}
              onClick={() => {
                const [a, b] = [...selected].sort((x, y) => x - y);
                setDiffPair({ from: a, to: b });
              }}
            >
              Compare selected
            </Button>
            {selected.length !== 2 ? <span>Select two versions to compare.</span> : null}
          </p>
          {diffPair ? (
            diff.isPending ? (
              <Skeleton lines={4} />
            ) : diff.isError ? (
              <ErrorState title="Could not load the diff" message="The diff may be too large, or a version is missing." />
            ) : diff.data ? (
              <DiffView diff={diff.data.diff} />
            ) : null
          ) : null}
        </div>
      ) : (
        <div style={{ display: 'grid', gap: 'var(--fh-space-3)' }}>
          {forks.isPending ? (
            <Skeleton lines={3} />
          ) : forks.isError ? (
            <ErrorState title="Could not load forks" message="Try again." retryLabel="Retry" onRetry={() => void forks.refetch()} />
          ) : (forks.data?.forks ?? []).length === 0 ? (
            <EmptyState
              icon="call_split"
              title="No forks yet"
              message={
                s.fork_count > 0
                  ? 'This snippet has forks, but none of them are visible to you (forks start private).'
                  : 'Nobody has forked this snippet yet. Forks are copies other people keep in their own libraries.'
              }
            />
          ) : (
            <Card variant="outlined">
              <md-list>
                {(forks.data?.forks ?? []).map((f) => (
                  <ListItem
                    key={`${f.owner}/${f.name}`}
                    headline={`${f.owner}/${f.name}`}
                    supportingText={`v${f.version} - ${relativeTime(f.updated_at)}`}
                    trailing={<StatusChip visibility={f.visibility} />}
                    onClick={() => navigate(`/${f.owner}/${f.name}`)}
                  />
                ))}
              </md-list>
            </Card>
          )}
        </div>
      )}

      {/* Delete: requires typing the name (owner only). */}
      <md-dialog open={deleteOpen || undefined} onClose={() => setDeleteOpen(false)}>
        <div slot="headline">Delete {name}?</div>
        <div slot="content">
          <p>
            Type the snippet name to confirm. It stays recoverable for 30 days, then it is deleted
            for good.
          </p>
          <TextField label={`Type "${name}"`} value={deleteName} onChange={setDeleteName} />
        </div>
        <div slot="actions">
          <DialogActions
            confirmLabel="Delete"
            danger
            confirmDisabled={deleteName !== name}
            onCancel={() => setDeleteOpen(false)}
            onConfirm={() =>
              deleteSnippet.mutate(
                { owner, name },
                {
                  onSuccess: () => {
                    toast({ message: `${name} deleted` });
                    navigate('/dashboard');
                  },
                  onError: (e) => toast({ message: e.uiMessage }),
                  onSettled: () => setDeleteOpen(false),
                },
              )
            }
          />
        </div>
      </md-dialog>

      {/* Fork dialog (handles the name-taken retry). */}
      <md-dialog open={forkOpen || undefined} onClose={() => setForkOpen(false)}>
        <div slot="headline">Fork {owner}/{name}</div>
        <div slot="content">
          <p>The fork lands in your account as a private snippet with its history noted.</p>
          <TextField label="Name in your account" value={forkName} onChange={setForkName} />
        </div>
        <div slot="actions">
          <DialogActions
            confirmLabel="Fork"
            confirmDisabled={forkName.trim() === ''}
            onCancel={() => setForkOpen(false)}
            onConfirm={() =>
              fork.mutate(
                { owner, name, newName: forkName },
                {
                  onSuccess: (created) => {
                    toast({ message: `Forked as ${created.owner}/${created.name}` });
                    setForkOpen(false);
                    navigate(`/${created.owner}/${created.name}`);
                  },
                  onError: (e) =>
                    toast({
                      message:
                        e.code === 'name_taken'
                          ? 'You already have a snippet with that name. Pick another.'
                          : e.uiMessage,
                    }),
                },
              )
            }
          />
        </div>
      </md-dialog>

      {/* Report dialog. */}
      <md-dialog open={reportOpen || undefined} onClose={() => setReportOpen(false)}>
        <div slot="headline">Report this snippet</div>
        <div slot="content">
          <p>Tell the moderators what is wrong. Reports are not shown to the snippet's owner.</p>
          <TextField label="Reason" value={reportReason} onChange={setReportReason} multiline rows={3} maxLength={500} />
        </div>
        <div slot="actions">
          <DialogActions
            confirmLabel="Send report"
            confirmDisabled={reportReason.trim() === ''}
            onCancel={() => setReportOpen(false)}
            onConfirm={() =>
              report.mutate(
                { owner, name, reason: reportReason.trim() },
                {
                  onSuccess: () => {
                    toast({ message: 'Report sent. Thank you.' });
                    setReportOpen(false);
                    setReportReason('');
                  },
                  onError: (e) => toast({ message: e.uiMessage }),
                },
              )
            }
          />
        </div>
      </md-dialog>
    </div>
  );
}
