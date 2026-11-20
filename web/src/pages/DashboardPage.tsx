import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Button,
  Card,
  CodeBlock,
  ConfirmDialog,
  EmptyState,
  ErrorState,
  Icon,
  Menu,
  PageHeader,
  SearchBar,
  Skeleton,
  StatusChip,
  useToast,
} from '../design';
import { ApiError, apiUrl } from '../api/client';
import {
  useDeleteSnippet,
  useMySnippets,
  useSetVisibility,
  type SnippetSort,
  type VisibilityFilter,
} from '../api/hooks';
import type { Snippet, Visibility } from '../api/types';
import { relativeTime } from '../validation';

const FILTERS: { id: VisibilityFilter; label: string }[] = [
  { id: 'all', label: 'All' },
  { id: 'private', label: 'Private' },
  { id: 'unlisted', label: 'Unlisted' },
  { id: 'public', label: 'Public' },
];
const SORTS: { id: SnippetSort; label: string }[] = [
  { id: 'updated', label: 'Recently updated' },
  { id: 'created', label: 'Recently created' },
  { id: 'name', label: 'Name' },
];
const VISIBILITIES: Visibility[] = ['private', 'unlisted', 'public'];

export default function DashboardPage() {
  const [filter, setFilter] = useState<VisibilityFilter>('all');
  const [sort, setSort] = useState<SnippetSort>('updated');
  const [query, setQuery] = useState('');
  const [toDelete, setToDelete] = useState<Snippet | null>(null);
  const snippets = useMySnippets(filter, sort);
  const deleteSnippet = useDeleteSnippet();
  const setVisibility = useSetVisibility();
  const toast = useToast();
  const navigate = useNavigate();

  // Session expired mid-visit: back to login, and return here.
  useEffect(() => {
    if (snippets.error instanceof ApiError && snippets.error.status === 401) {
      navigate('/login?return=/dashboard', { replace: true });
    }
  }, [snippets.error, navigate]);

  const visible = useMemo(() => {
    const all = snippets.data?.snippets ?? [];
    const q = query.trim().toLowerCase();
    if (!q) return all;
    return all.filter(
      (s) =>
        s.name.toLowerCase().includes(q) ||
        s.description.toLowerCase().includes(q) ||
        s.tags.some((t) => t.toLowerCase().includes(q)),
    );
  }, [snippets.data, query]);

  const copy = async (text: string, message: string) => {
    try {
      await navigator.clipboard.writeText(text);
      toast({ message });
    } catch {
      toast({ message: 'Copy failed - your browser blocked clipboard access' });
    }
  };

  const onMenu = (snippet: Snippet, action: string) => {
    if (action === 'copy-run') {
      void copy(`fg ${snippet.name}`, `Copied: fg ${snippet.name}`);
    } else if (action === 'copy-raw') {
      void copy(
        `${window.location.origin}${apiUrl(`/snippets/${snippet.owner}/${snippet.name}/raw`)}`,
        'Raw URL copied',
      );
    } else if (action === 'delete') {
      setToDelete(snippet);
    } else if (action.startsWith('visibility:')) {
      const visibility = action.slice('visibility:'.length) as Visibility;
      setVisibility.mutate(
        { owner: snippet.owner, name: snippet.name, visibility },
        {
          onSuccess: () => toast({ message: `${snippet.name} is now ${visibility}` }),
          onError: (error) => toast({ message: error.uiMessage }),
        },
      );
    }
  };

  return (
    <div>
      <PageHeader
        title="Your snippets"
        subtitle="Everything you have saved, private by default."
        actions={
          <Button variant="filled" icon="add" onClick={() => navigate('/new')}>
            New snippet
          </Button>
        }
      />

      <div style={{ display: 'grid', gap: 'var(--fh-space-3)', marginBottom: 'var(--fh-space-4)' }}>
        <SearchBar value={query} onChange={setQuery} placeholder="Search your snippets" ariaLabel="Search your snippets" />
        <div style={{ display: 'flex', gap: 'var(--fh-space-3)', alignItems: 'center', flexWrap: 'wrap' }}>
          <md-chip-set aria-label="Filter by visibility">
            {FILTERS.map((f) => (
              <md-filter-chip
                key={f.id}
                label={f.label}
                selected={filter === f.id || undefined}
                onClick={() => setFilter(f.id)}
              />
            ))}
          </md-chip-set>
          <span style={{ flex: 1 }} />
          <Menu
            buttonLabel="Sort snippets"
            buttonIcon="sort"
            items={SORTS.map((s) => ({ id: s.id, label: s.label }))}
            onSelect={(id) => setSort(id as SnippetSort)}
          />
        </div>
      </div>

      {snippets.isPending ? (
        <Card variant="outlined">
          <Skeleton lines={4} />
        </Card>
      ) : snippets.isError ? (
        <ErrorState
          title="Could not load your snippets"
          message={snippets.error instanceof ApiError ? snippets.error.uiMessage : 'Something went wrong.'}
          retryLabel="Retry"
          onRetry={() => void snippets.refetch()}
        />
      ) : visible.length === 0 && !query && filter === 'all' ? (
        <div style={{ display: 'grid', gap: 'var(--fh-space-4)' }}>
          <EmptyState
            icon="library_books"
            title="No snippets yet"
            message="Save the commands you keep re-typing. They stay private until you choose to share them."
            actionLabel="New snippet"
            onAction={() => navigate('/new')}
          />
          <Card variant="filled">
            <h2 style={{ marginTop: 0 }}>Run them from your terminal</h2>
            <p>
              With the <code>fg</code> CLI, a saved snippet runs anywhere after you confirm exactly
              what it will do:
            </p>
            <CodeBlock label="Terminal" code={'fg login\nfg ubuntu-setup'} />
            <p style={{ color: 'var(--md-sys-color-on-surface-variant)' }}>
              Install instructions live in the ForgeHub README (installer packages arrive with the
              first release).
            </p>
          </Card>
        </div>
      ) : visible.length === 0 ? (
        <EmptyState icon="search" title="Nothing matches" message="No snippets match this filter and search. Try clearing one of them." />
      ) : (
        <Card variant="outlined">
          <md-list>
            {visible.map((snippet) => (
              <div key={snippet.name} style={{ display: 'flex', alignItems: 'center' }}>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <md-list-item>
                    <span slot="headline">{snippet.name}</span>
                    <span slot="supporting-text">
                      {snippet.description || 'No description'} · v{snippet.version} ·{' '}
                      <span title={new Date(snippet.updated_at).toLocaleString()}>
                        {relativeTime(snippet.updated_at)}
                      </span>
                    </span>
                    <span slot="start">
                      <Icon name={snippet.summary?.mode === 'steps' ? 'library_books' : 'terminal'} />
                    </span>
                  </md-list-item>
                  {snippet.tags.length > 0 ? (
                    <div style={{ display: 'flex', gap: 'var(--fh-space-1)', flexWrap: 'wrap', padding: '0 var(--fh-space-4) var(--fh-space-2)' }}>
                      {snippet.tags.map((tag) => (
                        <md-assist-chip key={tag} label={tag} />
                      ))}
                    </div>
                  ) : null}
                </div>
                <StatusChip visibility={snippet.visibility} />
                <Menu
                  buttonLabel={`Actions for ${snippet.name}`}
                  items={[
                    { id: 'copy-run', label: 'Copy run command', icon: 'content_copy' },
                    { id: 'copy-raw', label: 'Copy raw URL', icon: 'link' },
                    ...VISIBILITIES.filter((v) => v !== snippet.visibility).map((v) => ({
                      id: `visibility:${v}`,
                      label: `Make ${v}`,
                      icon: v === 'public' ? 'public' : v === 'unlisted' ? 'link' : 'lock',
                    })),
                    { id: 'delete', label: 'Delete', icon: 'delete' },
                  ]}
                  onSelect={(id) => onMenu(snippet, id)}
                />
              </div>
            ))}
          </md-list>
        </Card>
      )}

      <ConfirmDialog
        open={toDelete !== null}
        headline={`Delete ${toDelete?.name ?? ''}?`}
        content="The snippet is removed from your library. Its name stays reserved for 30 days, then it is deleted for good."
        confirmLabel="Delete"
        danger
        onCancel={() => setToDelete(null)}
        onConfirm={() => {
          if (!toDelete) return;
          deleteSnippet.mutate(
            { owner: toDelete.owner, name: toDelete.name },
            {
              onSuccess: () => toast({ message: `${toDelete.name} deleted` }),
              onError: (error) => toast({ message: error.uiMessage }),
              onSettled: () => setToDelete(null),
            },
          );
        }}
      />
    </div>
  );
}
