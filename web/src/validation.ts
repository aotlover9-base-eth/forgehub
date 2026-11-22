/**
 * Client-side validation hints mirroring the server rules
 * (docs/spec-snippet-format.md; forge-core names.rs). The
 * server is always the authority - these exist so the user
 * gets feedback while typing, not after submitting.
 */

import { fmt, m } from './i18n/messages';

// Kept in sync with RESERVED_WORDS in crates/forge-core/src/names.rs.
export const RESERVED_USERNAMES = new Set(
  'run init record login logout whoami push pull fork status diff list show search tag rename clone delete edit import export analyze parameterize stats backup restore doctor alias completions ui cache update trust remote help version config me api admin settings explore new device signup static assets health tokens users snippets'.split(
    ' ',
  ),
);

export function usernameIssue(username: string): string | null {
  if (username.length === 0) return null; // no nagging before typing
  if (username.length < 2) return m.validation.atLeast2;
  if (username.length > 39) return m.validation.atMost39;
  if (!/^[a-z0-9-]+$/.test(username)) return m.validation.lowercaseDigitsHyphens;
  if (!/^[a-z0-9]/.test(username)) return m.validation.startLetterOrDigit;
  if (username.endsWith('-')) return m.validation.noEndHyphen;
  if (username.includes('--')) return m.validation.noDoubleHyphen;
  if (RESERVED_USERNAMES.has(username)) return m.validation.nameReserved;
  return null;
}

export function passwordIssue(password: string, username: string): string | null {
  if (password.length === 0) return null;
  if (password.length < 10) return m.validation.atLeast10;
  if (password.length > 256) return m.validation.atMost256;
  if (username && password === username) return m.validation.passwordNotUsername;
  return null;
}

export function emailIssue(email: string): string | null {
  if (email.length === 0) return null;
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) return m.validation.emailInvalid;
  return null;
}

/** Snippet-name rules (forge-core validate_snippet_name):
 * 1–64 chars, starts lowercase alnum, then [a-z0-9._-],
 * not a reserved word. */
export function snippetNameIssue(name: string): string | null {
  if (name.length === 0) return null;
  if (name.length > 64) return m.validation.atMost64;
  if (!/^[a-z0-9]/.test(name)) return m.validation.startLowercaseOrDigit;
  if (!/^[a-z0-9._-]+$/.test(name)) return m.validation.snippetChars;
  if (RESERVED_USERNAMES.has(name)) return 'That name is reserved.';
  return null;
}

/** Tag rules (forge-core normalize_tag): lowercase, trimmed,
 * [a-z0-9._-] after the first char, at most 50 chars. */
export function tagIssue(tag: string): string | null {
  const t = tag.trim().toLowerCase();
  if (t.length === 0) return m.validation.tagEmpty;
  if (t.length > 50) return m.validation.atMost50;
  if (!/^[a-z0-9]/.test(t)) return m.validation.tagStart;
  if (!/^[a-z0-9._-]+$/.test(t)) return 'Lowercase letters, digits, dots, underscores and hyphens only.';
  return null;
}

/** "3h ago" style relative time, with the full date nearby
 * in a title attribute at the call site. */
export function relativeTime(iso: string, now = Date.now()): string {
  const seconds = Math.max(0, Math.floor((now - new Date(iso).getTime()) / 1000));
  if (seconds < 60) return m.validation.justNow;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return fmt(m.validation.minutesAgo, { count: minutes });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return fmt(m.validation.hoursAgo, { count: hours });
  const days = Math.floor(hours / 24);
  if (days < 30) return fmt(m.validation.daysAgo, { count: days });
  return new Date(iso).toLocaleDateString();
}
