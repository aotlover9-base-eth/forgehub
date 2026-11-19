/**
 * Client-side validation hints mirroring the server rules
 * (docs/spec-snippet-format.md; forge-core names.rs). The
 * server is always the authority — these exist so the user
 * gets feedback while typing, not after submitting.
 */

// Kept in sync with RESERVED_WORDS in crates/forge-core/src/names.rs.
export const RESERVED_USERNAMES = new Set(
  'run init record login logout whoami push pull fork status diff list show search tag rename clone delete edit import export analyze parameterize stats backup restore doctor alias completions ui cache update trust remote help version config me api admin settings explore new device signup static assets health tokens users snippets'.split(
    ' ',
  ),
);

export function usernameIssue(username: string): string | null {
  if (username.length === 0) return null; // no nagging before typing
  if (username.length < 2) return 'At least 2 characters.';
  if (username.length > 39) return 'At most 39 characters.';
  if (!/^[a-z0-9-]+$/.test(username)) return 'Lowercase letters, digits and hyphens only.';
  if (!/^[a-z0-9]/.test(username)) return 'Must start with a letter or digit.';
  if (username.endsWith('-')) return 'Cannot end with a hyphen.';
  if (username.includes('--')) return 'No consecutive hyphens.';
  if (RESERVED_USERNAMES.has(username)) return 'That name is reserved.';
  return null;
}

export function passwordIssue(password: string, username: string): string | null {
  if (password.length === 0) return null;
  if (password.length < 10) return 'At least 10 characters.';
  if (password.length > 256) return 'At most 256 characters.';
  if (username && password === username) return 'Password cannot be your username.';
  return null;
}

export function emailIssue(email: string): string | null {
  if (email.length === 0) return null;
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) return 'That does not look like an email address.';
  return null;
}

/** "3h ago" style relative time, with the full date nearby
 * in a title attribute at the call site. */
export function relativeTime(iso: string, now = Date.now()): string {
  const seconds = Math.max(0, Math.floor((now - new Date(iso).getTime()) / 1000));
  if (seconds < 60) return 'just now';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  return new Date(iso).toLocaleDateString();
}
