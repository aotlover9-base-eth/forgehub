import { useEffect, useMemo, useState, type ReactNode } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import {
  Button,
  Card,
  Chip,
  EmptyState,
  ErrorState,
  Icon,
  PageHeader,
  SearchBar,
  SegmentedControl,
  Skeleton,
} from '../design';
import { ApiError } from '../api/client';
import { useExplore, usePopularTags, useSearch, type SearchSort } from '../api/hooks';
import type { SearchResult, Snippet } from '../api/types';
import { relativeTime } from '../validation';

/**
 * Render text with the API's byte-range highlights wrapped
 * in <mark>. Ranges are UTF-8 byte offsets; we walk code
 * points so non-ASCII text lines up. Everything renders as
 * React text - no HTML is ever injected.
 */
export function highlighted(text: string, ranges: { start: number; end: number }[]): ReactNode {
  if (ranges.length === 0) return text;
  const sorted = [...ranges].sort((a, b) => a.start - b.start);
  const encoder = new TextEncoder();
  const parts: ReactNode[] = [];
  let bytePos = 0;
  let rangeIdx = 0;
  let buffer = '';
  let inMark = false;
  let key = 0;
  const flush = () => {
    if (buffer) {
      parts.push(inMark ? <mark key={key++}>{buffer}</mark> : <span key={key++}>{buffer}</span>);
      buffer = '';
    }
  };
  for (const ch of text) {
    while (rangeIdx < sorted.length && bytePos >= sorted[rangeIdx].end) rangeIdx++;
    const active = rangeIdx < sorted.length && bytePos >= sorted[rangeIdx].start && bytePos < sorted[rangeIdx].end;
    if (active !== inMark) {
      flush();
      inMark = active;
    }
    buffer += ch;
    bytePos += encoder.encode(ch).length;
  }
  flush();
  return parts;
}

function SnippetCard({ snippet, nameNode, descNode }: { snippet: Snippet; nameNode?: ReactNode; descNode?: ReactNode }) {
  return (
    <Card variant="outlined">
      <p style={{ margin: 0, fontWeight: 500 }}>
        <Link to={`/${snippet.owner}/${snippet.name}`} style={{ color: 'inherit' }}>
          {nameNode ?? snippet.name}
        </Link>{' '}
        <span style={{ color: 'var(--md-sys-color-on-surface-variant)', fontWeight: 400 }}>
          by {snippet.owner}
        </span>
      </p>
      <p style={{ margin: 'var(--fh-space-1) 0', color: 'var(--md-sys-color-on-surface-variant)' }}>
        {descNode ?? (snippet.description || 'No description.')}
      </p>
      <p style={{ display: 'flex', gap: 'var(--fh-space-3)', alignItems: 'center', flexWrap: 'wrap', margin: 0 }}>
        <span>
          <Icon name="star" /> {snippet.star_count}
        </span>
        <span>
          <Icon name="call_split" /> {snippet.fork_count}
        </span>
        <span title={new Date(snippet.updated_at).toLocaleString()}>{relativeTime(snippet.updated_at)}</span>
        {snippet.tags.map((tag) => (
          <Chip key={tag} label={tag} />
        ))}
      </p>
    </Card>
  );
}

const SORTS: { id: SearchSort; label: string }[] = [
  { id: 'relevance', label: 'Relevance' },
  { id: 'stars', label: 'Most stars' },
  { id: 'updated', label: 'Recent' },
];

export default function ExplorePage() {
  // The app bar search entry lands here with ?q=; adopt it
  // as the initial (and updated) query.
  const [searchParams] = useSearchParams();
  const [query, setQuery] = useState(() => searchParams.get('q') ?? '');
  // Adopt a new ?q= when the URL changes while this page
  // stays mounted. Adjusting state during render (guarded
  // by the previous params) is the React-endorsed
  // replacement for a sync-from-props effect; it fires
  // under exactly the condition the old effect did - the
  // searchParams object changing - and before paint.
  const [seenParams, setSeenParams] = useState(searchParams);
  if (searchParams !== seenParams) {
    setSeenParams(searchParams);
    const q = searchParams.get('q');
    if (q !== null) setQuery(q);
  }
  const [debounced, setDebounced] = useState('');
  const [tag, setTag] = useState<string | null>(null);
  const [sort, setSort] = useState<SearchSort>('relevance');
  const [feed, setFeed] = useState<'recent' | 'popular'>('recent');
  useEffect(() => {
    const timer = window.setTimeout(() => setDebounced(query), 350);
    return () => window.clearTimeout(timer);
  }, [query]);

  const searching = debounced.trim().length > 0 || tag !== null;
  const search = useSearch(debounced, tag, sort);
  const explore = useExplore(feed);
  const popularTags = usePopularTags();

  const results: SearchResult[] = useMemo(
    () => search.data?.pages.flatMap((p) => p.results) ?? [],
    [search.data],
  );
  const feedItems: Snippet[] = useMemo(
    () => explore.data?.pages.flatMap((p) => p.snippets) ?? [],
    [explore.data],
  );

  return (
    <div style={{ display: 'grid', gap: 'var(--fh-space-4)' }}>
      <PageHeader title="Explore" subtitle="Public snippets from everyone on ForgeHub." />
      <SearchBar value={query} onChange={setQuery} placeholder="Search public snippets" ariaLabel="Search public snippets" />

      {popularTags.data && popularTags.data.tags.length > 0 ? (
        <div style={{ display: 'flex', gap: 'var(--fh-space-1)', flexWrap: 'wrap', alignItems: 'center' }}>
          <span style={{ color: 'var(--md-sys-color-on-surface-variant)' }}>Popular:</span>
          {popularTags.data.tags.slice(0, 10).map(({ tag: t, count }) => (
            <span key={t} style={{ display: 'inline-flex', alignItems: 'center', gap: 'var(--fh-space-1)' }}>
              <Chip label={`${t} (${count})`} onClick={() => setTag(tag === t ? null : t)} />
            </span>
          ))}
          {tag ? (
            <Button variant="text" onClick={() => setTag(null)}>
              Clear tag filter: {tag}
            </Button>
          ) : null}
        </div>
      ) : null}

      {searching ? (
        <>
          <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--fh-space-2)' }}>
            <span style={{ flex: 1 }} />
            <SegmentedControl ariaLabel="Sort results" options={SORTS} value={sort} onChange={setSort} />
          </div>
          {search.isPending ? (
            <Skeleton lines={4} />
          ) : search.isError ? (
            <ErrorState
              title="Search failed"
              message={search.error instanceof ApiError ? search.error.uiMessage : 'Something went wrong.'}
              retryLabel="Retry"
              onRetry={() => void search.refetch()}
            />
          ) : results.length === 0 ? (
            <EmptyState icon="search" title="No results" message="Nothing public matches that yet. Try fewer words, or a different tag." />
          ) : (
            <>
              <div style={{ display: 'grid', gap: 'var(--fh-space-3)' }}>
                {results.map(({ snippet, highlights }) => (
                  <SnippetCard
                    key={`${snippet.owner}/${snippet.name}`}
                    snippet={snippet}
                    nameNode={highlighted(snippet.name, highlights.filter((h) => h.field === 'name'))}
                    descNode={
                      snippet.description
                        ? highlighted(snippet.description, highlights.filter((h) => h.field === 'description'))
                        : undefined
                    }
                  />
                ))}
              </div>
              {search.hasNextPage ? (
                <p>
                  <Button variant="tonal" onClick={() => void search.fetchNextPage()} disabled={search.isFetchingNextPage}>
                    {search.isFetchingNextPage ? 'Loading…' : 'Load more'}
                  </Button>
                </p>
              ) : null}
            </>
          )}
        </>
      ) : (
        <>
          <div style={{ display: 'flex', gap: 'var(--fh-space-2)' }}>
            <md-chip-set aria-label="Feed">
              <md-filter-chip label="Recent" selected={feed === 'recent' || undefined} onClick={() => setFeed('recent')} />
              <md-filter-chip label="Popular this week" selected={feed === 'popular' || undefined} onClick={() => setFeed('popular')} />
            </md-chip-set>
          </div>
          {explore.isPending ? (
            <Skeleton lines={4} />
          ) : explore.isError ? (
            <ErrorState
              title="Could not load Explore"
              message={explore.error instanceof ApiError ? explore.error.uiMessage : 'Something went wrong.'}
              retryLabel="Retry"
              onRetry={() => void explore.refetch()}
            />
          ) : feedItems.length === 0 ? (
            <EmptyState icon="explore" title="Nothing public yet" message="When people share snippets publicly, they will show up here." />
          ) : (
            <>
              <div style={{ display: 'grid', gap: 'var(--fh-space-3)' }}>
                {feedItems.map((snippet) => (
                  <SnippetCard key={`${snippet.owner}/${snippet.name}`} snippet={snippet} />
                ))}
              </div>
              {explore.hasNextPage ? (
                <p>
                  <Button variant="tonal" onClick={() => void explore.fetchNextPage()} disabled={explore.isFetchingNextPage}>
                    {explore.isFetchingNextPage ? 'Loading…' : 'Load more'}
                  </Button>
                </p>
              ) : null}
            </>
          )}
        </>
      )}
    </div>
  );
}
