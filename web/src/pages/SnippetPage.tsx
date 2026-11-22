import { useMemo, useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import {
  Button,
  Card,
  Checkbox,
  Chip,
  CodeBlock,
  CopyIconButton,
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
  useRollbackSnippet,
  useSetVisibility,
  useSnippet,
  useSnippetVersion,
  useStar,
  useForks,
  useVersions,
} from '../api/hooks';
import type { Visibility } from '../api/types';
import { relativeTime } from '../validation';
import { fmt, m } from '../i18n/messages';

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
  const [rollbackTarget, setRollbackTarget] = useState<number | null>(null);
  const rollback = useRollbackSnippet();
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
        title={m.snippet.notFoundTitle}
        message={m.snippet.notFoundMessage}
        actionLabel="Go to Explore"
        onAction={() => navigate('/explore')}
      />
    );
  }
  const s = snippet.data;


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
            {s.description || m.snippet.noDescription}
          </p>
          <p style={{ display: 'flex', gap: 'var(--fh-space-3)', alignItems: 'center', flexWrap: 'wrap', margin: 0 }}>
            <StatusChip visibility={s.visibility} />
            <span>
              <Icon name="star" filled={s.starred_by_me} /> {s.star_count}
            </span>
            <span>
              <Icon name="call_split" /> {fmt(m.snippet.forksCount, { count: s.fork_count })}
            </span>
            <span>
              <Icon name="download" /> {fmt(m.snippet.pullsCount, { count: s.pull_count })}
            </span>
            <span title={new Date(s.updated_at).toLocaleString()}>{fmt(m.snippet.versionUpdated, { version: s.version, when: relativeTime(s.updated_at) })}</span>
          </p>
          {s.forked_from ? (
            <p style={{ color: 'var(--md-sys-color-on-surface-variant)' }}>
              {m.snippet.forkedFrom}{' '}
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
                {m.snippet.edit}
              </Button>
              <Menu
                buttonLabel={m.snippet.actions}
                items={[
                  ...VISIBILITIES.filter((v) => v !== s.visibility).map((v) => ({
                    id: `visibility:${v}`,
                    label: fmt(m.library.makeVisibility, { visibility: v }),
                    icon: v === 'public' ? 'public' : v === 'unlisted' ? 'link' : 'lock',
                  })),
                  { id: 'delete', label: m.snippet.delete, icon: 'delete' },
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
                        onSuccess: () => toast({ message: fmt(m.snippet.nowVisibility, { name, visibility }) }),
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
                {s.starred_by_me ? m.snippet.starred : m.snippet.star}
              </Button>
              <Button
                variant="outlined"
                icon="call_split"
                onClick={() => {
                  setForkName(name);
                  setForkOpen(true);
                }}
              >
                {m.snippet.fork}
              </Button>
              <IconButton icon="report" label={m.snippet.reportLabel} onClick={() => setReportOpen(true)} />
            </>
          ) : null}
          <a href={apiUrl(`/snippets/${owner}/${name}/raw`)} target="_blank" rel="noreferrer" aria-label={m.snippet.rawAria}>
            <Button variant="text">{m.snippet.raw}</Button>
          </a>
        </div>
      </div>

      {!own && s.visibility === 'public' ? (
        <Card variant="filled">
          <p style={{ margin: 0, display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center' }}>
            <Icon name="shield" />
            {m.snippet.reviewBefore} <code>fg</code> {m.snippet.reviewAfter}
          </p>
        </Card>
      ) : null}

      <Card variant="outlined">
        <h2 style={{ marginTop: 0 }}>{m.snippet.runIt}</h2>
        <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center' }}>
          <code style={{ flex: 1, fontSize: 'var(--md-sys-typescale-body-large-size)' }}>{runCommand}</code>
          <CopyIconButton text={runCommand} label={m.snippet.copyRun} onCopied={() => toast({ message: fmt(m.snippet.copiedCommand, { command: runCommand }) })} />
        </div>
        <p style={{ color: 'var(--md-sys-color-on-surface-variant)', marginBottom: 0 }}>
          {m.snippet.newToFgBefore} <code>fg</code>{m.snippet.newToFgAfter}
        </p>
        {s.visibility === 'public' ? (
          <p style={{ marginBottom: 0 }}>
            <a href={`/s/${owner}/${name}`} target="_blank" rel="noreferrer">
              {m.snippet.publicPage}
            </a>{' '}
            {m.snippet.publicPageNote}
          </p>
        ) : null}
      </Card>

      <Tabs
        ariaLabel={m.snippet.sectionsAria}
        tabs={[
          { id: 'code', label: m.snippet.tabCode },
          { id: 'versions', label: m.snippet.tabVersions },
          { id: 'forks', label: m.snippet.tabForks },
        ]}
        activeIndex={tab}
        onChange={setTab}
      />

      {tab === 0 ? (
        <div style={{ display: 'grid', gap: 'var(--fh-space-3)' }}>
          {viewVersion !== null ? (
            <p>
              {fmt(m.snippet.viewingVersion, { version: viewVersion })}{' '}
              <Button variant="text" onClick={() => setViewVersion(null)}>
                {fmt(m.snippet.backToLatest, { version: s.version })}
              </Button>
            </p>
          ) : null}
          {shown?.summary && shown.summary.lint_warnings.length > 0 ? (
            <Card variant="filled">
              <p style={{ margin: 0, fontWeight: 500 }}>{m.snippet.warnings}</p>
              <ul>
                {shown.summary.lint_warnings.map((w, i) => (
                  <li key={i}>
                    {fmt(m.snippet.lintLine, { line: w.line, message: w.message })}
                  </li>
                ))}
              </ul>
              {shown.summary.uses_sudo ? <p style={{ margin: 0 }}>{m.snippet.usesSudo}</p> : null}
            </Card>
          ) : shown?.summary?.uses_sudo ? (
            <p>
              <Icon name="warning" /> {m.snippet.usesSudo}
            </p>
          ) : null}
          <CodeBlock code={shown?.body ?? ''} lineNumbers label={fmt(m.snippet.codeLabel, { name, version: shown?.version ?? s.version })} />
        </div>
      ) : tab === 1 ? (
        <div style={{ display: 'grid', gap: 'var(--fh-space-3)' }}>
          {versions.isPending ? (
            <Skeleton lines={3} />
          ) : versions.isError ? (
            <ErrorState title={m.snippet.versionsFailed} message={m.snippet.tryAgain} retryLabel={m.common.retry} onRetry={() => void versions.refetch()} />
          ) : (
            <Card variant="outlined">
              <md-list>
                {(versions.data?.versions ?? []).map((v) => (
                  <div key={v.version} style={{ display: 'flex', alignItems: 'center', gap: 'var(--fh-space-2)' }}>
                    <Checkbox
                      ariaLabel={fmt(m.snippet.selectVersionAria, { version: v.version })}
                      checked={selected.includes(v.version)}
                      onChange={() => toggleVersion(v.version)}
                    />
                    <div style={{ flex: 1 }}>
                      <md-list-item>
                        <span slot="headline">
                          {fmt(m.snippet.versionLine, { version: v.version, hash: v.short_hash })}
                        </span>
                        <span slot="supporting-text">
                          {v.message || m.snippet.noMessage} ·{' '}
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
                      {m.snippet.viewCode}
                    </Button>
                    {own && v.version !== s.version ? (
                      <Button variant="text" onClick={() => setRollbackTarget(v.version)}>
                        {m.snippet.rollBack}
                      </Button>
                    ) : null}
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
              {m.snippet.compareSelected}
            </Button>
            {selected.length !== 2 ? <span>{m.snippet.selectTwo}</span> : null}
          </p>
          {diffPair ? (
            diff.isPending ? (
              <Skeleton lines={4} />
            ) : diff.isError ? (
              <ErrorState title={m.snippet.diffFailed} message={m.snippet.diffFailedMessage} />
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
            <ErrorState title={m.snippet.forksFailed} message={m.snippet.tryAgain} retryLabel={m.common.retry} onRetry={() => void forks.refetch()} />
          ) : (forks.data?.forks ?? []).length === 0 ? (
            <EmptyState
              icon="call_split"
              title={m.snippet.noForksTitle}
              message={
                s.fork_count > 0
                  ? m.snippet.noForksHidden
                  : m.snippet.noForksNone
              }
            />
          ) : (
            <Card variant="outlined">
              <md-list>
                {(forks.data?.forks ?? []).map((f) => (
                  <ListItem
                    key={`${f.owner}/${f.name}`}
                    headline={`${f.owner}/${f.name}`}
                    supportingText={fmt(m.snippet.forkLine, { version: f.version, when: relativeTime(f.updated_at) })}
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
        <div slot="headline">{fmt(m.snippet.deleteHeadline, { name })}</div>
        <div slot="content">
          <p>
            {m.snippet.deleteBody}
          </p>
          <TextField label={fmt(m.snippet.typeName, { name })} value={deleteName} onChange={setDeleteName} />
        </div>
        <div slot="actions">
          <DialogActions
            confirmLabel={m.snippet.delete}
            danger
            confirmDisabled={deleteName !== name}
            onCancel={() => setDeleteOpen(false)}
            onConfirm={() =>
              deleteSnippet.mutate(
                { owner, name },
                {
                  onSuccess: () => {
                    toast({ message: fmt(m.snippet.deletedToast, { name }) });
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

      {/* Rollback: publishes the old content as a new version (Peak C3). */}
      <md-dialog open={rollbackTarget !== null || undefined} onClose={() => setRollbackTarget(null)}>
        <div slot="headline">{fmt(m.snippet.rollbackHeadline, { version: rollbackTarget ?? '' })}</div>
        <div slot="content">
          <p>
            {fmt(m.snippet.rollbackBodyBefore, { next: s.version + 1, target: rollbackTarget ?? '' })}&apos;s{fmt(m.snippet.rollbackBodyAfter, { current: s.version })}
          </p>
        </div>
        <div slot="actions">
          <DialogActions
            confirmLabel={m.snippet.rollBack}
            onCancel={() => setRollbackTarget(null)}
            onConfirm={() => {
              if (rollbackTarget === null) return;
              rollback.mutate(
                { owner, name, ifMatch: s.version, version: rollbackTarget },
                {
                  onSuccess: (data) =>
                    toast({ message: fmt(m.snippet.rolledBackToast, { version: data.version }) }),
                  onError: (e) => toast({ message: e.uiMessage }),
                  onSettled: () => setRollbackTarget(null),
                },
              );
            }}
          />
        </div>
      </md-dialog>

      {/* Fork dialog (handles the name-taken retry). */}
      <md-dialog open={forkOpen || undefined} onClose={() => setForkOpen(false)}>
        <div slot="headline">{fmt(m.snippet.forkHeadline, { owner, name })}</div>
        <div slot="content">
          <p>{m.snippet.forkBody}</p>
          <TextField label={m.snippet.forkNameLabel} value={forkName} onChange={setForkName} />
        </div>
        <div slot="actions">
          <DialogActions
            confirmLabel={m.snippet.fork}
            confirmDisabled={forkName.trim() === ''}
            onCancel={() => setForkOpen(false)}
            onConfirm={() =>
              fork.mutate(
                { owner, name, newName: forkName },
                {
                  onSuccess: (created) => {
                    toast({ message: fmt(m.snippet.forkedToast, { owner: created.owner, name: created.name }) });
                    setForkOpen(false);
                    navigate(`/${created.owner}/${created.name}`);
                  },
                  onError: (e) =>
                    toast({
                      message:
                        e.code === 'name_taken'
                          ? m.snippet.forkNameTaken
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
        <div slot="headline">{m.snippet.reportHeadline}</div>
        <div slot="content">
          <p>{m.snippet.reportBody}</p>
          <TextField label={m.snippet.reportReason} value={reportReason} onChange={setReportReason} multiline rows={3} maxLength={500} />
        </div>
        <div slot="actions">
          <DialogActions
            confirmLabel={m.snippet.reportConfirm}
            confirmDisabled={reportReason.trim() === ''}
            onCancel={() => setReportOpen(false)}
            onConfirm={() =>
              report.mutate(
                { owner, name, reason: reportReason.trim() },
                {
                  onSuccess: () => {
                    toast({ message: m.snippet.reportSent });
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
