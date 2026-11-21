import { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Button,
  Card,
  Checkbox,
  CodeBlock,
  CopyIconButton,
  ConfirmDialog,
  DialogActions,
  EmptyState,
  ErrorState,
  Icon,
  IconButton,
  ListItem,
  Menu,
  PageHeader,
  SearchBar,
  Skeleton,
  StatusChip,
  TextField,
  useDensity,
  useToast,
  useWidthClass,
} from '../design';
import { ApiError, apiUrl } from '../api/client';
import {
  useDeleteSnippet,
  useMySnippets,
  usePatchSnippet,
  useSetVisibility,
  useSnippet,
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

/** True when a keydown lands in a field (shadow-DOM aware). */
function isTypingEvent(e: KeyboardEvent): boolean {
  return e.composedPath().some((node) => {
    if (!(node instanceof HTMLElement)) return false;
    return (
      node instanceof HTMLInputElement ||
      node instanceof HTMLTextAreaElement ||
      node instanceof HTMLSelectElement ||
      node.isContentEditable ||
      node.tagName.endsWith('TEXT-FIELD')
    );
  });
}

export default function DashboardPage() {
  const [filter, setFilter] = useState<VisibilityFilter>('all');
  const [sort, setSort] = useState<SnippetSort>('updated');
  const [query, setQuery] = useState('');
  const [toDelete, setToDelete] = useState<Snippet | null>(null);
  const [bulkMode, setBulkMode] = useState(false);
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [bulkDeleteOpen, setBulkDeleteOpen] = useState(false);
  const [tagOpen, setTagOpen] = useState(false);
  const [tagDraft, setTagDraft] = useState('');
  const [cursor, setCursor] = useState(0);
  const [sheetName, setSheetName] = useState<string | null>(null);
  const snippets = useMySnippets(filter, sort);
  const deleteSnippet = useDeleteSnippet();
  const setVisibility = useSetVisibility();
  const patchSnippet = usePatchSnippet();
  const toast = useToast();
  const navigate = useNavigate();
  const widthClass = useWidthClass();
  const [density, setDensity] = useDensity();
  const listRef = useRef<HTMLDivElement>(null);

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

  // Keep the keyboard cursor inside the visible list.
  useEffect(() => {
    setCursor((c) => Math.min(c, Math.max(0, visible.length - 1)));
  }, [visible.length]);

  const sheetSnippet = visible.find((s) => s.name === sheetName) ?? null;
  const sheetDetail = useSnippet(sheetSnippet?.owner ?? '', sheetName ?? '', sheetName !== null);

  const copy = async (text: string, message: string) => {
    try {
      await navigator.clipboard.writeText(text);
      toast({ message });
    } catch {
      toast({ message: 'Copy failed - your browser blocked clipboard access' });
    }
  };

  const openSnippet = (snippet: Snippet) => {
    if (bulkMode) {
      setSelected((prev) => {
        const next = new Set(prev);
        if (next.has(snippet.name)) next.delete(snippet.name);
        else next.add(snippet.name);
        return next;
      });
      return;
    }
    // On expanded, a click previews in the side sheet; the
    // full page is one more click. Everywhere else it navigates.
    if (widthClass === 'expanded') {
      setSheetName(snippet.name);
    } else {
      navigate(`/${snippet.owner}/${snippet.name}`);
    }
  };

  // j/k move the cursor, Enter opens (the global handler in
  // App owns n, / and ?).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (isTypingEvent(e) || e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key === 'j') {
        e.preventDefault();
        setCursor((c) => Math.min(c + 1, Math.max(0, visible.length - 1)));
      } else if (e.key === 'k') {
        e.preventDefault();
        setCursor((c) => Math.max(c - 1, 0));
      } else if (e.key === 'Enter') {
        const target = visible[cursor];
        if (target) {
          e.preventDefault();
          openSnippet(target);
        }
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
    // openSnippet is re-created each render; the listener
    // intentionally rebinds when the inputs change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [visible, cursor, bulkMode, widthClass]);

  useEffect(() => {
    listRef.current
      ?.querySelector('[data-cursor="true"]')
      ?.scrollIntoView({ block: 'nearest' });
  }, [cursor]);

  const onMenu = (snippet: Snippet, action: string) => {
    if (action === 'copy-run') {
      void copy(`fg ${snippet.name}`, `Copied: fg ${snippet.name}`);
    } else if (action === 'copy-raw') {
      void copy(
        `${window.location.origin}${apiUrl(`/snippets/${snippet.owner}/${snippet.name}/raw`)}`,
        'Raw URL copied',
      );
    } else if (action === 'open') {
      navigate(`/${snippet.owner}/${snippet.name}`);
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

  const selectedSnippets = visible.filter((s) => selected.has(s.name));

  const runBulk = async (label: string, run: (s: Snippet) => Promise<unknown>) => {
    let failed = 0;
    for (const snippet of selectedSnippets) {
      try {
        await run(snippet);
      } catch {
        failed += 1;
      }
    }
    const done = selectedSnippets.length - failed;
    toast({
      message:
        failed === 0
          ? `${label}: ${done} snippet${done === 1 ? '' : 's'} done`
          : `${label}: ${done} done, ${failed} failed - the list has been refreshed`,
    });
    setSelected(new Set());
    setBulkMode(false);
  };

  const total = snippets.data?.snippets.length ?? 0;

  return (
    <div style={{ display: 'flex', gap: 'var(--fh-space-4)', alignItems: 'flex-start' }}>
      <div style={{ flex: 1, minWidth: 0 }}>
        <PageHeader
          title="Your snippets"
          subtitle={
            snippets.data
              ? `${total} snippet${total === 1 ? '' : 's'} saved - private by default.`
              : 'Everything you have saved, private by default.'
          }
          actions={
            <Button variant="filled" icon="add" onClick={() => navigate('/new')}>
              New snippet
            </Button>
          }
        />

        <div style={{ display: 'grid', gap: 'var(--fh-space-3)', marginBottom: 'var(--fh-space-4)' }}>
          <SearchBar value={query} onChange={setQuery} placeholder="Search your snippets" ariaLabel="Search your snippets" />
          <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center', flexWrap: 'wrap' }}>
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
            <Button
              variant={bulkMode ? 'tonal' : 'text'}
              icon="check"
              onClick={() => {
                setBulkMode(!bulkMode);
                setSelected(new Set());
              }}
            >
              Select
            </Button>
            <Button
              variant="text"
              onClick={() => setDensity(density === 'compact' ? 'comfortable' : 'compact')}
            >
              {density === 'compact' ? 'Density: Compact' : 'Density: Comfortable'}
            </Button>
            <Menu
              buttonLabel="Sort snippets"
              buttonIcon="sort"
              items={SORTS.map((s) => ({ id: s.id, label: s.label }))}
              onSelect={(id) => setSort(id as SnippetSort)}
            />
          </div>
        </div>

        {bulkMode ? (
          <Card variant="filled">
            <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center', flexWrap: 'wrap' }}>
              <strong>
                {selected.size} selected
              </strong>
              <span style={{ flex: 1 }} />
              <Menu
                buttonLabel="Set visibility for selected"
                buttonIcon="visibility"
                items={VISIBILITIES.map((v) => ({ id: v, label: `Make ${v}` }))}
                onSelect={(id) =>
                  void runBulk('Visibility set', (s) =>
                    setVisibility.mutateAsync({
                      owner: s.owner,
                      name: s.name,
                      visibility: id as Visibility,
                    }),
                  )
                }
              />
              <Button variant="text" icon="sell" disabled={selected.size === 0} onClick={() => setTagOpen(true)}>
                Add tag
              </Button>
              <Button
                variant="text"
                icon="delete"
                disabled={selected.size === 0}
                onClick={() => setBulkDeleteOpen(true)}
              >
                Delete
              </Button>
              <IconButton
                icon="close"
                label="Leave bulk select"
                onClick={() => {
                  setBulkMode(false);
                  setSelected(new Set());
                }}
              />
            </div>
          </Card>
        ) : null}

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
            <div ref={listRef} className="fh-list-enter">
              <md-list>
                {visible.map((snippet, index) => (
                  <div
                    key={snippet.name}
                    data-cursor={index === cursor ? 'true' : undefined}
                    className={`fh-list-row${index === cursor ? ' fh-list-row--cursor' : ''}`}
                    style={{ display: 'flex', alignItems: 'center' }}
                  >
                    {bulkMode ? (
                      <Checkbox
                        ariaLabel={`Select ${snippet.name}`}
                        checked={selected.has(snippet.name)}
                        onChange={() => openSnippet(snippet)}
                      />
                    ) : null}
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <ListItem
                        headline={snippet.name}
                        supportingText={`${snippet.description || 'No description'} - v${snippet.version} - ${relativeTime(snippet.updated_at)}`}
                        leading={
                          <Icon name={snippet.summary?.mode === 'steps' ? 'library_books' : 'terminal'} />
                        }
                        onClick={() => {
                          setCursor(index);
                          openSnippet(snippet);
                        }}
                      />
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
                        { id: 'open', label: 'Open', icon: 'chevron_right' },
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
            </div>
          </Card>
        )}
      </div>

      {sheetSnippet ? (
        <aside
          className="fh-sheet-enter"
          aria-label={`Preview of ${sheetSnippet.name}`}
          style={{ width: 360, flexShrink: 0, position: 'sticky', top: 0 }}
        >
          <Card variant="outlined">
            <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--fh-space-2)' }}>
              <h2 style={{ margin: 0, flex: 1 }}>{sheetSnippet.name}</h2>
              <IconButton icon="close" label="Close preview" onClick={() => setSheetName(null)} />
            </div>
            <p style={{ color: 'var(--md-sys-color-on-surface-variant)' }}>
              {sheetSnippet.description || 'No description'} - v{sheetSnippet.version}
            </p>
            {sheetDetail.isPending ? (
              <Skeleton lines={4} />
            ) : sheetDetail.data ? (
              <div style={{ display: 'grid', gap: 'var(--fh-space-3)' }}>
                <CodeBlock code={sheetDetail.data.body} label={`${sheetSnippet.name} - v${sheetDetail.data.version}`} />
                <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center' }}>
                  <code style={{ flex: 1 }}>fg {sheetSnippet.name}</code>
                  <CopyIconButton
                    text={`fg ${sheetSnippet.name}`}
                    label="Copy run command"
                    onCopied={() => toast({ message: `Copied: fg ${sheetSnippet.name}` })}
                  />
                </div>
                <Button
                  variant="filled"
                  onClick={() => navigate(`/${sheetSnippet.owner}/${sheetSnippet.name}`)}
                >
                  Open full page
                </Button>
              </div>
            ) : (
              <ErrorState title="Could not load the preview" message="Open the full page instead." />
            )}
          </Card>
        </aside>
      ) : null}

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

      <ConfirmDialog
        open={bulkDeleteOpen}
        headline={`Delete ${selected.size} snippet${selected.size === 1 ? '' : 's'}?`}
        content="They are removed from your library. Names stay reserved for 30 days, then they are deleted for good."
        confirmLabel="Delete"
        danger
        onCancel={() => setBulkDeleteOpen(false)}
        onConfirm={() => {
          setBulkDeleteOpen(false);
          void runBulk('Deleted', (s) => deleteSnippet.mutateAsync({ owner: s.owner, name: s.name }));
        }}
      />

      <md-dialog open={tagOpen || undefined} onClose={() => setTagOpen(false)}>
        <div slot="headline">Add a tag to {selected.size} snippets</div>
        <div slot="content">
          <TextField
            label="Tag"
            value={tagDraft}
            onChange={setTagDraft}
            supportingText="Lowercase letters, digits and hyphens. Snippets that already have it are skipped."
          />
        </div>
        <div slot="actions">
          <DialogActions
            confirmLabel="Add tag"
            confirmDisabled={tagDraft.trim() === ''}
            onCancel={() => setTagOpen(false)}
            onConfirm={() => {
              const tag = tagDraft.trim().toLowerCase();
              setTagOpen(false);
              setTagDraft('');
              void runBulk('Tag added', (s) =>
                s.tags.includes(tag)
                  ? Promise.resolve(s)
                  : patchSnippet.mutateAsync({
                      owner: s.owner,
                      name: s.name,
                      patch: { tags: [...s.tags, tag] },
                    }),
              );
            }}
          />
        </div>
      </md-dialog>
    </div>
  );
}
