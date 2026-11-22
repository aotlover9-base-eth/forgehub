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
  useAddToCollection,
  useDeleteSnippet,
  useMyCollections,
  useMySnippets,
  usePatchSnippet,
  useSetVisibility,
  useSnippet,
  type SnippetSort,
  type VisibilityFilter,
} from '../api/hooks';
import CollectionsPanel from './CollectionsPanel';
import ImportPanel from './ImportPanel';
import LibraryStats from './LibraryStats';
import type { Snippet, Visibility } from '../api/types';
import { relativeTime } from '../validation';
import { fmt, m } from '../i18n/messages';

const FILTERS: { id: VisibilityFilter; label: string }[] = [
  { id: 'all', label: m.library.filterAll },
  { id: 'private', label: m.design.visibilityPrivate },
  { id: 'unlisted', label: m.design.visibilityUnlisted },
  { id: 'public', label: m.design.visibilityPublic },
];
const SORTS: { id: SnippetSort; label: string }[] = [
  { id: 'updated', label: m.library.sortUpdated },
  { id: 'created', label: m.library.sortCreated },
  { id: 'name', label: m.library.sortName },
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
  const myCollections = useMyCollections();
  const addToCollection = useAddToCollection();
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
  // Adjusting state during render (guarded by the
  // previous length) is the React-endorsed replacement
  // for a clamp effect; the cursor ends up exactly
  // where the effect put it, without the extra render.
  const [clampedLength, setClampedLength] = useState(visible.length);
  if (clampedLength !== visible.length) {
    setClampedLength(visible.length);
    setCursor((c) => Math.min(c, Math.max(0, visible.length - 1)));
  }

  const sheetSnippet = visible.find((s) => s.name === sheetName) ?? null;
  const sheetDetail = useSnippet(sheetSnippet?.owner ?? '', sheetName ?? '', sheetName !== null);

  const copy = async (text: string, message: string) => {
    try {
      await navigator.clipboard.writeText(text);
      toast({ message });
    } catch {
      toast({ message: m.library.copyFailed });
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
      void copy(`fg ${snippet.name}`, fmt(m.library.copiedFg, { name: snippet.name }));
    } else if (action === 'copy-raw') {
      void copy(
        `${window.location.origin}${apiUrl(`/snippets/${snippet.owner}/${snippet.name}/raw`)}`,
        m.library.rawUrlCopied,
      );
    } else if (action === 'open') {
      navigate(`/${snippet.owner}/${snippet.name}`);
    } else if (action === 'delete') {
      setToDelete(snippet);
    } else if (action.startsWith('collection:')) {
      const collectionName = action.slice('collection:'.length);
      addToCollection.mutate(
        {
          owner: snippet.owner,
          name: collectionName,
          snippetOwner: snippet.owner,
          snippetName: snippet.name,
        },
        {
          onSuccess: () => toast({ message: fmt(m.library.addedTo, { name: snippet.name, collection: collectionName }) }),
          onError: (error) => toast({ message: error.uiMessage }),
        },
      );
    } else if (action.startsWith('visibility:')) {
      const visibility = action.slice('visibility:'.length) as Visibility;
      setVisibility.mutate(
        { owner: snippet.owner, name: snippet.name, visibility },
        {
          onSuccess: () => toast({ message: fmt(m.library.nowVisibility, { name: snippet.name, visibility }) }),
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
          ? fmt(m.library.bulkDone, { label, done, word: done === 1 ? m.library.savedOne : m.library.savedMany })
          : fmt(m.library.bulkPartial, { label, done, failed }),
    });
    setSelected(new Set());
    setBulkMode(false);
  };

  const total = snippets.data?.snippets.length ?? 0;

  return (
    <div style={{ display: 'flex', gap: 'var(--fh-space-4)', alignItems: 'flex-start' }}>
      <div style={{ flex: 1, minWidth: 0 }}>
        <PageHeader
          title={m.library.title}
          subtitle={
            snippets.data
              ? fmt(m.library.countLine, { count: total, word: total === 1 ? m.library.savedOne : m.library.savedMany })
              : m.library.subtitleFallback
          }
          actions={
            <Button variant="filled" icon="add" onClick={() => navigate('/new')}>
              {m.library.newSnippet}
            </Button>
          }
        />

        <div style={{ display: 'grid', gap: 'var(--fh-space-3)', marginBottom: 'var(--fh-space-4)' }}>
          <SearchBar value={query} onChange={setQuery} placeholder={m.library.searchPlaceholder} ariaLabel={m.library.searchPlaceholder} />
          <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center', flexWrap: 'wrap' }}>
            <md-chip-set aria-label={m.library.filterAria}>
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
              {m.library.select}
            </Button>
            <Button
              variant="text"
              onClick={() => setDensity(density === 'compact' ? 'comfortable' : 'compact')}
            >
              {density === 'compact' ? m.library.densityCompact : m.library.densityComfortable}
            </Button>
            <Menu
              buttonLabel={m.library.sortLabel}
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
                {fmt(m.library.selectedCount, { count: selected.size })}
              </strong>
              <span style={{ flex: 1 }} />
              <Menu
                buttonLabel={m.library.setVisibilitySelected}
                buttonIcon="visibility"
                items={VISIBILITIES.map((v) => ({ id: v, label: fmt(m.library.makeVisibility, { visibility: v }) }))}
                onSelect={(id) =>
                  void runBulk(m.library.bulkVisibilityLabel, (s) =>
                    setVisibility.mutateAsync({
                      owner: s.owner,
                      name: s.name,
                      visibility: id as Visibility,
                    }),
                  )
                }
              />
              <Button variant="text" icon="sell" disabled={selected.size === 0} onClick={() => setTagOpen(true)}>
                {m.library.addTag}
              </Button>
              <Button
                variant="text"
                icon="delete"
                disabled={selected.size === 0}
                onClick={() => setBulkDeleteOpen(true)}
              >
                {m.library.delete}
              </Button>
              <IconButton
                icon="close"
                label={m.library.leaveBulk}
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
            title={m.library.loadFailed}
            message={snippets.error instanceof ApiError ? snippets.error.uiMessage : m.common.somethingWrongShort}
            retryLabel={m.common.retry}
            onRetry={() => void snippets.refetch()}
          />
        ) : visible.length === 0 && !query && filter === 'all' ? (
          <div style={{ display: 'grid', gap: 'var(--fh-space-4)' }}>
            <EmptyState
              icon="library_books"
              title={m.library.emptyTitle}
              message={m.library.emptyMessage}
              actionLabel={m.library.newSnippet}
              onAction={() => navigate('/new')}
            />
            <Card variant="filled">
              <h2 style={{ marginTop: 0 }}>{m.library.runFromTerminalTitle}</h2>
              <p>
                {m.library.runFromTerminalBodyBefore} <code>fg</code> {m.library.runFromTerminalBodyAfter}
              </p>
              <CodeBlock label={m.home.terminal} code={'fg login\nfg ubuntu-setup'} />
              <p style={{ color: 'var(--md-sys-color-on-surface-variant)' }}>
                {m.library.installNote}
              </p>
            </Card>
          </div>
        ) : visible.length === 0 ? (
          <EmptyState icon="search" title={m.library.nothingTitle} message={m.library.nothingMessage} />
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
                        ariaLabel={fmt(m.library.selectAria, { name: snippet.name })}
                        checked={selected.has(snippet.name)}
                        onChange={() => openSnippet(snippet)}
                      />
                    ) : null}
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <ListItem
                        headline={snippet.name}
                        supportingText={fmt(m.library.listLine, { description: snippet.description || m.library.noDescription, version: snippet.version, when: relativeTime(snippet.updated_at) })}
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
                      buttonLabel={fmt(m.library.actionsFor, { name: snippet.name })}
                      items={[
                        { id: 'open', label: m.library.menuOpen, icon: 'chevron_right' },
                        { id: 'copy-run', label: m.library.menuCopyRun, icon: 'content_copy' },
                        { id: 'copy-raw', label: m.library.menuCopyRaw, icon: 'link' },
                        ...VISIBILITIES.filter((v) => v !== snippet.visibility).map((v) => ({
                          id: `visibility:${v}`,
                          label: fmt(m.library.makeVisibility, { visibility: v }),
                          icon: v === 'public' ? 'public' : v === 'unlisted' ? 'link' : 'lock',
                        })),
                        ...(myCollections.data?.collections ?? []).map((c) => ({
                          id: `collection:${c.name}`,
                          label: fmt(m.library.menuAddTo, { name: c.name }),
                          icon: 'playlist_add',
                        })),
                        { id: 'delete', label: m.library.menuDelete, icon: 'delete' },
                      ]}
                      onSelect={(id) => onMenu(snippet, id)}
                    />
                  </div>
                ))}
              </md-list>
            </div>
          </Card>
        )}

        <div style={{ marginTop: 'var(--fh-space-4)' }}>
          <LibraryStats />
        </div>
        <div style={{ marginTop: 'var(--fh-space-4)' }}>
          <CollectionsPanel />
        </div>
        <div style={{ marginTop: 'var(--fh-space-4)' }}>
          <ImportPanel />
        </div>
      </div>

      {sheetSnippet ? (
        <aside
          className="fh-sheet-enter"
          aria-label={fmt(m.library.previewOf, { name: sheetSnippet.name })}
          style={{ width: 360, flexShrink: 0, position: 'sticky', top: 0 }}
        >
          <Card variant="outlined">
            <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--fh-space-2)' }}>
              <h2 style={{ margin: 0, flex: 1 }}>{sheetSnippet.name}</h2>
              <IconButton icon="close" label={m.library.closePreview} onClick={() => setSheetName(null)} />
            </div>
            <p style={{ color: 'var(--md-sys-color-on-surface-variant)' }}>
              {fmt(m.library.previewLine, { description: sheetSnippet.description || m.library.noDescription, version: sheetSnippet.version })}
            </p>
            {sheetDetail.isPending ? (
              <Skeleton lines={4} />
            ) : sheetDetail.data ? (
              <div style={{ display: 'grid', gap: 'var(--fh-space-3)' }}>
                <CodeBlock code={sheetDetail.data.body} label={fmt(m.library.previewCodeLabel, { name: sheetSnippet.name, version: sheetDetail.data.version })} />
                <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center' }}>
                  <code style={{ flex: 1 }}>fg {sheetSnippet.name}</code>
                  <CopyIconButton
                    text={`fg ${sheetSnippet.name}`}
                    label={m.library.menuCopyRun}
                    onCopied={() => toast({ message: fmt(m.library.copiedFg, { name: sheetSnippet.name }) })}
                  />
                </div>
                <Button
                  variant="filled"
                  onClick={() => navigate(`/${sheetSnippet.owner}/${sheetSnippet.name}`)}
                >
                  {m.library.openFullPage}
                </Button>
              </div>
            ) : (
              <ErrorState title={m.library.previewFailedTitle} message={m.library.previewFailedMessage} />
            )}
          </Card>
        </aside>
      ) : null}

      <ConfirmDialog
        open={toDelete !== null}
        headline={fmt(m.library.deleteOneHeadline, { name: toDelete?.name ?? '' })}
        content={m.library.deleteOneContent}
        confirmLabel={m.library.delete}
        danger
        onCancel={() => setToDelete(null)}
        onConfirm={() => {
          if (!toDelete) return;
          deleteSnippet.mutate(
            { owner: toDelete.owner, name: toDelete.name },
            {
              onSuccess: () => toast({ message: fmt(m.library.deletedToast, { name: toDelete.name }) }),
              onError: (error) => toast({ message: error.uiMessage }),
              onSettled: () => setToDelete(null),
            },
          );
        }}
      />

      <ConfirmDialog
        open={bulkDeleteOpen}
        headline={fmt(m.library.deleteManyHeadline, { count: selected.size, word: selected.size === 1 ? m.library.savedOne : m.library.savedMany })}
        content={m.library.deleteManyContent}
        confirmLabel="Delete"
        danger
        onCancel={() => setBulkDeleteOpen(false)}
        onConfirm={() => {
          setBulkDeleteOpen(false);
          void runBulk(m.library.bulkDeletedLabel, (s) => deleteSnippet.mutateAsync({ owner: s.owner, name: s.name }));
        }}
      />

      <md-dialog open={tagOpen || undefined} onClose={() => setTagOpen(false)}>
        <div slot="headline">{fmt(m.library.tagDialogHeadline, { count: selected.size })}</div>
        <div slot="content">
          <TextField
            label={m.library.tagLabel}
            value={tagDraft}
            onChange={setTagDraft}
            supportingText={m.library.tagHint}
          />
        </div>
        <div slot="actions">
          <DialogActions
            confirmLabel={m.library.tagConfirm}
            confirmDisabled={tagDraft.trim() === ''}
            onCancel={() => setTagOpen(false)}
            onConfirm={() => {
              const tag = tagDraft.trim().toLowerCase();
              setTagOpen(false);
              setTagDraft('');
              void runBulk(m.library.bulkTagLabel, (s) =>
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
