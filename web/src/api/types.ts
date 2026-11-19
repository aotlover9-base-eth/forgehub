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

export interface SnippetSummaryParsed {
  mode: 'script' | 'steps';
  steps_count: number;
  variables: string[];
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

export interface ApiErrorBody {
  error: { code: string; message: string; details?: unknown };
}
