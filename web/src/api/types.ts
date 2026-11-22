/**
 * API types, hand-written to match docs/openapi.yaml (no
 * offline generator is available; keep in sync with the spec).
 */

export type Visibility = 'private' | 'unlisted' | 'public';

export interface Me {
  username: string;
  email: string;
  display_name: string | null;
  bio: string | null;
  created_at: string;
  /** Present only for cookie sessions; needed for mutations. */
  csrf_token?: string;
}

/** One declared parameter (Peak C2 typed parameters). */
export interface ParamInfo {
  name: string;
  type: 'text' | 'choice' | 'number' | 'path' | 'yesno' | 'secret';
  required: boolean;
  default?: string;
  choices?: string[];
  description?: string;
}

export interface SnippetSummaryParsed {
  mode: 'script' | 'steps';
  steps_count: number;
  variables: string[];
  /** Declared parameters with types; absent from older servers. */
  parameters?: ParamInfo[];
  uses_sudo: boolean;
  lint_warnings: { kind: string; line: number; message: string }[];
}

export interface Snippet {
  owner: string;
  name: string;
  description: string;
  visibility: Visibility;
  tags: string[];
  version: number;
  hash: string;
  short_hash: string;
  body: string;
  message: string | null;
  summary?: SnippetSummaryParsed;
  star_count: number;
  fork_count: number;
  pull_count: number;
  forked_from: { owner: string; name: string; version: number; hash: string } | null;
  starred_by_me: boolean;
  created_at: string;
  updated_at: string;
}

export interface SnippetPage {
  snippets: Snippet[];
  next_cursor: string | null;
}

/** A snippet collection (Peak C1). */
export interface Collection {
  owner: string;
  name: string;
  description: string;
  visibility: Visibility;
  item_count: number;
  created_at: string;
  updated_at: string;
}

/** One item of a collection detail response. */
export interface CollectionItem {
  owner: string;
  name: string;
  description: string;
  visibility: Visibility;
  latest_version: number;
  position: number;
}

/** Per-snippet run aggregates (Peak C8). */
export interface WorkflowStat {
  ref: string;
  runs: number;
  commands: number;
  total_ms: number;
  failures: number;
  estimated_saved_ms: number;
  last_run: string;
}

/** The caller's library stats: only their own cloud runs. */
export interface MyStats {
  runs: number;
  commands: number;
  total_ms: number;
  failures: number;
  estimated_saved_ms: number;
  workflows: WorkflowStat[];
}

/** One activity-log event (Peak C7). `detail` is a small
 * object whose shape depends on `kind`. */
export interface ActivityEvent {
  id: number;
  kind: string;
  detail: Record<string, unknown>;
  created_at: string;
}

/** A collection share link as the owner sees it (Peak C6,
 * ADR 0011). The full token is never in this shape - it is
 * returned once, at creation, in `CreatedShareLink`. */
export interface ShareLink {
  id: number;
  prefix: string;
  created_at: string;
  expires_at: string;
  revoked_at: string | null;
  active: boolean;
}

/** The create response: the one-time full URL plus the
 * management view of the link. */
export interface CreatedShareLink extends ShareLink {
  url: string;
  token: string;
}

/** One mined history candidate (Peak C5). */
export interface HistoryCandidate {
  name: string;
  commands: string[];
  count: number;
}

/** Result of POST /imports/history/analyze. */
export interface HistoryAnalysis {
  parsed: number;
  kept: number;
  candidates: HistoryCandidate[];
}

/** Result of POST /imports/gist. */
export interface GistImportResult {
  created: { name: string; file: string }[];
  skipped: { file: string; reason: string }[];
}

/** Collection detail: the collection plus its visible items
 * in order. `total_items` is present only for the owner and
 * counts items the server hid from this view. */
export interface CollectionDetail extends Collection {
  items: CollectionItem[];
  total_items?: number;
}

export interface ApiErrorBody {
  error: { code: string; message: string; details?: unknown };
}

export interface ValidationResult {
  valid: boolean;
  errors: { message: string; line?: number; column?: number; path?: string }[];
  hash?: string;
  summary?: SnippetSummaryParsed;
  secret_findings?: { kind: string; line: number }[];
}

export interface VersionSummary {
  version: number;
  hash: string;
  short_hash: string;
  message: string | null;
  created_at: string;
}

export interface SnippetInput {
  name: string;
  description?: string;
  visibility?: Visibility;
  tags?: string[];
  mode?: 'script' | 'steps';
  body: string;
  message?: string;
}

export interface SearchResult {
  snippet: Snippet;
  highlights: { field: string; start: number; end: number }[];
}

export interface SearchPage {
  results: SearchResult[];
  next_cursor: string | null;
}

export interface PublicProfile {
  username: string;
  display_name: string | null;
  bio: string | null;
  created_at: string;
  public_snippet_count: number;
}

export interface SessionInfo {
  ref: string;
  created_at: string;
  last_used_at: string;
  expires_at: string;
  current: boolean;
}

export interface ApiTokenInfo {
  id: number;
  name: string;
  prefix: string;
  scopes: string[];
  created_at: string;
  last_used_at: string | null;
  expires_at: string | null;
  revoked_at: string | null;
}

/** POST /tokens response: the metadata plus the plaintext,
 * which the server shows exactly once. */
export interface CreatedToken extends ApiTokenInfo {
  token: string;
}
