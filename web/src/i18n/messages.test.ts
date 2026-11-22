import { describe, expect, it } from 'vitest';
import { catalogues, en, fmt } from './messages';

/**
 * Catalogue integrity (Peak C9): every leaf is a non-empty
 * string, and every locale catalogue (only English exists
 * today) carries exactly the same keys as English, so a
 * future translation cannot silently drop or invent a
 * string. fmt fills known placeholders and leaves unknown
 * ones untouched.
 */

function leaves(node: unknown, prefix: string, out: Map<string, string>): void {
  if (typeof node === 'string') {
    out.set(prefix, node);
    return;
  }
  if (node !== null && typeof node === 'object') {
    for (const [key, value] of Object.entries(node as Record<string, unknown>)) {
      leaves(value, prefix ? `${prefix}.${key}` : key, out);
    }
    return;
  }
  throw new Error(`catalogue leaf at ${prefix} is not a string`);
}

describe('message catalogue', () => {
  it('every English leaf is a non-empty string', () => {
    const map = new Map<string, string>();
    leaves(en, '', map);
    expect(map.size).toBeGreaterThan(400);
    for (const [key, value] of map) {
      expect(value.trim().length, key).toBeGreaterThan(0);
    }
  });

  it('every locale has exactly the English keys', () => {
    const english = new Map<string, string>();
    leaves(en, '', english);
    for (const [locale, catalogue] of Object.entries(catalogues)) {
      const map = new Map<string, string>();
      leaves(catalogue, '', map);
      expect([...map.keys()].sort(), locale).toEqual([...english.keys()].sort());
    }
  });

  it('fmt fills placeholders and leaves unknown ones alone', () => {
    expect(fmt('Hello {name}, {count} new', { name: 'ada', count: 3 })).toBe('Hello ada, 3 new');
    expect(fmt('Hello {name} {missing}', { name: 'ada' })).toBe('Hello ada {missing}');
    expect(fmt('No placeholders')).toBe('No placeholders');
  });
});
