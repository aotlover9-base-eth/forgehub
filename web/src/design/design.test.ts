import { describe, expect, it } from 'vitest';
import { SEED, darkColors, lightColors } from './tokens';
import { widthClassFor } from './hooks';

// WCAG 2.x relative luminance + contrast ratio.
function luminance(hex: string): number {
  const channels = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const linear = channels.map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
}
function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

describe('generated tokens', () => {
  it('come from the locked seed colour', () => {
    expect(SEED).toBe('#36618E');
    // Tonal Spot keeps the seed as the light primary.
    expect(lightColors['primary']).toBe('#36618e');
  });

  it('meet WCAG AA (4.5:1) for every text pair the UI uses, in both schemes', () => {
    // Peak A1: extended from the original nine pairs to every
    // foreground/background combination components actually
    // render text with (measured against the generated tokens).
    const pairs: [string, string][] = [
      ['on-surface', 'surface'],
      ['on-surface', 'surface-dim'],
      ['on-surface', 'surface-container-low'],
      ['on-surface', 'surface-container'],
      ['on-surface', 'surface-container-high'],
      ['on-surface', 'surface-container-highest'],
      ['on-surface-variant', 'surface'],
      ['on-surface-variant', 'surface-container-low'],
      ['on-surface-variant', 'surface-container-highest'],
      ['on-primary', 'primary'],
      ['on-secondary', 'secondary'],
      ['on-tertiary', 'tertiary'],
      ['on-primary-container', 'primary-container'],
      ['on-secondary-container', 'secondary-container'],
      ['on-tertiary-container', 'tertiary-container'],
      ['on-error', 'error'],
      ['on-error-container', 'error-container'],
      ['primary', 'surface'],
      ['primary', 'surface-container-low'],
      ['inverse-on-surface', 'inverse-surface'],
      ['inverse-primary', 'inverse-surface'],
    ];
    for (const [name, colors] of [
      ['light', lightColors],
      ['dark', darkColors],
    ] as const) {
      for (const [fg, bg] of pairs) {
        const ratio = contrast(colors[fg], colors[bg]);
        expect(ratio, `${name} ${fg} on ${bg}`).toBeGreaterThanOrEqual(4.5);
      }
    }
  });

  it('meet WCAG AA non-text contrast (3:1) for the outline role', () => {
    for (const [name, colors] of [
      ['light', lightColors],
      ['dark', darkColors],
    ] as const) {
      const ratio = contrast(colors['outline'], colors['surface']);
      expect(ratio, `${name} outline on surface`).toBeGreaterThanOrEqual(3);
    }
  });
});

describe('width classes (design-spec breakpoints)', () => {
  it('maps widths to compact / medium / expanded', () => {
    expect(widthClassFor(320)).toBe('compact');
    expect(widthClassFor(599)).toBe('compact');
    expect(widthClassFor(600)).toBe('medium');
    expect(widthClassFor(839)).toBe('medium');
    expect(widthClassFor(840)).toBe('expanded');
    expect(widthClassFor(1440)).toBe('expanded');
  });
});

