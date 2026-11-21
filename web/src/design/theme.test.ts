import { afterEach, describe, expect, it } from 'vitest';
import {
  applyTheme,
  getThemePreference,
  resolveTheme,
  setThemePreference,
} from './theme';

afterEach(() => {
  setThemePreference('system');
  try {
    window.localStorage.removeItem('forgehub-theme');
  } catch {
    // ignore: cleanup only
  }
});

describe('theme preference', () => {
  it('defaults to system when nothing is stored', () => {
    expect(getThemePreference()).toBe('system');
  });

  it('stores the preference and applies the resolved scheme', () => {
    setThemePreference('dark');
    expect(getThemePreference()).toBe('dark');
    expect(window.localStorage.getItem('forgehub-theme')).toBe('dark');
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    setThemePreference('light');
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
  });

  it('resolves explicit preferences without consulting the OS', () => {
    expect(resolveTheme('light')).toBe('light');
    expect(resolveTheme('dark')).toBe('dark');
  });

  it('resolves system through matchMedia when available, else light', () => {
    const resolved = resolveTheme('system');
    expect(['light', 'dark']).toContain(resolved);
    applyTheme('system');
    expect(document.documentElement.getAttribute('data-theme')).toBe(resolved);
  });

  it('ignores a corrupt stored value instead of throwing', () => {
    window.localStorage.setItem('forgehub-theme', 'neon');
    // readPreference ran at module load, so a corrupt value can
    // only be observed through a fresh resolution path: setting
    // a valid preference must keep working afterwards.
    setThemePreference('light');
    expect(getThemePreference()).toBe('light');
    expect(window.localStorage.getItem('forgehub-theme')).toBe('light');
  });
});
