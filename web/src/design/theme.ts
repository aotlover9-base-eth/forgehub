/**
 * Theme preference (system / light / dark), persisted in
 * localStorage and applied as `data-theme` on <html>. The
 * pre-paint inline script in index.html applies the same
 * logic before first paint; this module is the runtime API.
 * All storage access is wrapped in try/catch (private modes,
 * disabled storage).
 */
export type ThemePreference = 'system' | 'light' | 'dark';

const STORAGE_KEY = 'forgehub-theme';

export function readThemePreference(): ThemePreference {
  try {
    const value = window.localStorage.getItem(STORAGE_KEY);
    if (value === 'light' || value === 'dark' || value === 'system') {
      return value;
    }
  } catch {
    // Storage unavailable: fall through to system.
  }
  return 'system';
}

export function applyTheme(preference: ThemePreference): void {
  const root = document.documentElement;
  if (preference === 'system') {
    root.removeAttribute('data-theme');
  } else {
    root.setAttribute('data-theme', preference);
  }
}

export function setThemePreference(preference: ThemePreference): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, preference);
  } catch {
    // Persistence is best-effort; the theme still applies.
  }
  applyTheme(preference);
}

/** The pre-paint script body, kept in sync with index.html. */
export const PREPAINT_SCRIPT = `(function(){try{var t=localStorage.getItem('forgehub-theme');if(t==='light'||t==='dark'){document.documentElement.setAttribute('data-theme',t);}}catch(e){}})();`;
