/// <reference types="node" />
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import * as design from './index';

// Vitest stubs CSS imports, so the stylesheets under test
// are read from disk; index.html likewise.
function read(relative: string): string {
  return readFileSync(fileURLToPath(new URL(relative, import.meta.url)), 'utf8');
}

const tokensCss = read('./tokens.css');
const observeCss = read('./observe.css');
const indexHtml = read('../../index.html');

// The restored design contract (2026-11-23, owner-directed):
// exactly ONE theme - the dark Observe theme. These tests pin
// the restoration the way theme.test.ts once pinned the Peak
// theme switch: dark pinned in index.html, no theme control
// exported by the design system, the monochrome .observe-app
// scope in tokens.css, and the glass scene in observe.css.

// WCAG 2.x relative luminance + contrast ratio (same helpers
// as design.test.ts).
function luminance(rgb: [number, number, number]): number {
  const linear = rgb.map((v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
}
function contrast(a: [number, number, number], b: [number, number, number]): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}
/** White at the given alpha over a black backdrop. */
function whiteOverBlack(alpha: number): [number, number, number] {
  const v = Math.round(255 * alpha);
  return [v, v, v];
}

describe('the restored Observe design', () => {
  it('pins the dark theme in index.html, with no theme bootstrap', () => {
    expect(indexHtml).toContain('<html lang="en" data-theme="dark">');
    expect(indexHtml).not.toContain('theme-init');
  });

  it('exports no theme preference control (the toggle is gone)', () => {
    expect('useThemePreference' in design).toBe(false);
    expect('setThemePreference' in design).toBe(false);
    expect('resolveTheme' in design).toBe(false);
  });

  it('defines the Observe scene tokens (monochrome, black backdrop)', () => {
    expect(tokensCss).toContain('--fh-observe-bg: #000000;');
    expect(tokensCss).toContain('--fh-observe-fg: #ffffff;');
    expect(tokensCss).toContain('--fh-observe-glass-bg: rgb(0 0 0 / 0.1);');
    expect(tokensCss).toContain('--fh-observe-scrim: rgb(0 0 0 / 0.32);');
  });

  it('scopes the app shell to the monochrome palette (white is the only accent)', () => {
    const scope = tokensCss.slice(tokensCss.lastIndexOf('.observe-app {'));
    expect(scope).toContain('--md-sys-color-primary: #ffffff;');
    expect(scope).toContain('--md-sys-color-on-primary: #000000;');
    expect(scope).toContain('--fh-nav-active: #ffffff;');
    expect(scope).toContain('--fh-nav-active-text: #000000;');
    expect(scope).toContain('--fh-focus-ring: #ffffff;');
    // Error is deliberately NOT remapped to white: red means
    // error in every theme.
    expect(scope).not.toContain('--md-sys-color-error:');
  });

  it('keeps the glass scene in observe.css (liquid glass + app backdrop)', () => {
    expect(observeCss).toContain('.liquid-glass {');
    expect(observeCss).toContain('.observe-app-bg {');
    expect(observeCss).toContain("background-image: url('/media/observe-bg.jpg');");
    expect(observeCss).toContain('.observe-display {');
    expect(observeCss).toContain("font-family: 'Instrument Serif', Georgia, serif;");
  });

  it('meets WCAG AA for the Observe text tones over the black scene', () => {
    const black: [number, number, number] = [0, 0, 0];
    // Full white body text.
    expect(contrast([255, 255, 255], black)).toBeGreaterThanOrEqual(4.5);
    // fg-80 (nav links, secondary text) and fg-70 (sub copy).
    expect(contrast(whiteOverBlack(0.8), black)).toBeGreaterThanOrEqual(4.5);
    expect(contrast(whiteOverBlack(0.7), black)).toBeGreaterThanOrEqual(4.5);
  });
});
