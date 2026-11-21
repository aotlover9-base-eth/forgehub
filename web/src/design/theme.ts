import { useSyncExternalStore } from 'react';

/**
 * The theme preference: 'system' (follow the OS), 'light' or
 * 'dark'. Persisted in localStorage under 'forgehub-theme'
 * (every access in try/catch - storage can be unavailable),
 * resolved to a concrete scheme and applied as data-theme on
 * <html>. public/theme-init.js runs the same resolution
 * before first paint; this module owns it afterwards and
 * re-resolves live when the OS scheme flips under 'system'.
 */
export type ThemePreference = 'system' | 'light' | 'dark';
export type ResolvedTheme = 'light' | 'dark';

const STORAGE_SLOT = 'forgehub-theme';
const listeners = new Set<() => void>();

function readPreference(): ThemePreference {
  try {
    const stored = window.localStorage.getItem(STORAGE_SLOT);
    if (stored === 'light' || stored === 'dark' || stored === 'system') {
      return stored;
    }
  } catch {
    // Storage unavailable: the system preference is the default.
  }
  return 'system';
}

let current: ThemePreference = readPreference();

export function resolveTheme(pref: ThemePreference): ResolvedTheme {
  if (pref === 'light' || pref === 'dark') return pref;
  if (typeof window !== 'undefined' && typeof window.matchMedia === 'function') {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }
  return 'light';
}

export function applyTheme(pref: ThemePreference): void {
  const resolved = resolveTheme(pref);
  document.documentElement.setAttribute('data-theme', resolved);
  document.documentElement.style.colorScheme = resolved;
}

export function getThemePreference(): ThemePreference {
  return current;
}

export function setThemePreference(pref: ThemePreference): void {
  current = pref;
  try {
    window.localStorage.setItem(STORAGE_SLOT, pref);
  } catch {
    // The preference still applies for this session.
  }
  applyTheme(pref);
  for (const listener of listeners) listener();
}

export function useThemePreference(): [ThemePreference, (pref: ThemePreference) => void] {
  const pref = useSyncExternalStore(
    (onChange) => {
      listeners.add(onChange);
      return () => {
        listeners.delete(onChange);
      };
    },
    () => current,
    () => current,
  );
  return [pref, setThemePreference];
}

// Under 'system', an OS scheme change re-resolves immediately.
if (typeof window !== 'undefined' && typeof window.matchMedia === 'function') {
  const query = window.matchMedia('(prefers-color-scheme: dark)');
  query.addEventListener?.('change', () => {
    if (current === 'system') applyTheme('system');
  });
}
