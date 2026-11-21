#!/usr/bin/env node
/**
 * Token discipline check (Step 14): outside the token files,
 * no source file may contain
 *   - hex colours (#rgb / #rrggbb / #rrggbbaa),
 *   - raw rgb()/rgba()/hsl()/hsla() colour functions,
 *   - px border-radius values,
 *   - font-size literals (px/rem/em/pt).
 * Colours, radii and font sizes must come from tokens.css
 * variables. Exempt: src/design/tokens.css (the generated
 * token definitions), src/design/fonts.css (font faces + the
 * icon font size), src/design/tokens.ts (generated values),
 * and this script's own directory.
 *
 * Wired into `npm run lint`. Exits 1 with file:line findings.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const webRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const srcRoot = join(webRoot, 'src');

const EXEMPT = new Set([
  'src/design/tokens.css',
  'src/design/fonts.css',
  'src/design/tokens.ts',
]);

const RULES = [
  { name: 'hex colour', pattern: /#[0-9a-fA-F]{3,8}\b/ },
  { name: 'raw colour function', pattern: /\b(?:rgb|rgba|hsl|hsla)\(/ },
  {
    name: 'px border-radius',
    pattern: /border-radius\s*:[^;}]*\dpx/,
  },
  {
    name: 'font-size literal',
    pattern: /font-size\s*:\s*[\d.]+(?:px|rem|em|pt)/,
  },
  {
    name: 'inline font-size literal',
    pattern: /fontSize\s*:\s*['"]?[\d.]+(?:px|rem|em|pt)/,
  },
];

function* walk(dir) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) {
      yield* walk(path);
    } else if (/\.(css|ts|tsx)$/.test(entry)) {
      yield path;
    }
  }
}

let failures = 0;
for (const path of walk(srcRoot)) {
  const rel = relative(webRoot, path);
  if (EXEMPT.has(rel)) continue;
  // Test files assert token VALUES (e.g. the seed hex); they
  // do not style anything, so they are out of scope.
  if (/\.test\.(ts|tsx)$/.test(rel)) continue;
  const lines = readFileSync(path, 'utf8').split('\n');
  lines.forEach((line, index) => {
    // Colour literals inside comments documenting the seed are
    // not styling; style rules are what matter. (The seed hex
    // itself is allowed in prose comments.)
    const isComment = line.trim().startsWith('*') || line.trim().startsWith('//');
    for (const rule of RULES) {
      if (rule.pattern.test(line)) {
        if (isComment && rule.name === 'hex colour') continue;
        console.error(`${rel}:${index + 1}: ${rule.name}: ${line.trim()}`);
        failures += 1;
      }
    }
  });
}

// Required-token check (Peak A2): the typescale weight and
// tracking tokens, the full M3 duration scale, the easing set
// and the state-layer opacities must exist in tokens.css -
// components reference them, and a regeneration that dropped
// them would fail silently in the browser. Values are checked
// where the M3 spec fixes them.
const tokensCss = readFileSync(join(srcRoot, 'design', 'tokens.css'), 'utf8');
const typeStyles = [
  'display-large',
  'display-medium',
  'display-small',
  'headline-large',
  'headline-medium',
  'headline-small',
  'title-large',
  'title-medium',
  'title-small',
  'body-large',
  'body-medium',
  'body-small',
  'label-large',
  'label-medium',
  'label-small',
];
const required = [];
for (const style of typeStyles) {
  required.push(`--md-sys-typescale-${style}-weight:`);
  required.push(`--md-sys-typescale-${style}-tracking:`);
}
for (const name of [
  'short1',
  'short2',
  'short3',
  'short4',
  'medium1',
  'medium2',
  'medium3',
  'medium4',
  'long1',
  'long2',
  'long3',
  'long4',
  'extra-long1',
  'extra-long2',
  'extra-long3',
  'extra-long4',
]) {
  required.push(`--md-sys-motion-duration-${name}:`);
}
for (const name of [
  'standard',
  'standard-decelerate',
  'standard-accelerate',
  'emphasized',
  'emphasized-decelerate',
  'emphasized-accelerate',
]) {
  required.push(`--md-sys-motion-easing-${name}:`);
}
required.push('--md-sys-motion-easing-emphasized-decelerate: cubic-bezier(0.05, 0.7, 0.1, 1)');
required.push('--md-sys-motion-easing-emphasized-accelerate: cubic-bezier(0.3, 0, 0.8, 0.15)');
required.push('--md-sys-state-hover-state-layer-opacity: 0.08');
required.push('--md-sys-state-focus-state-layer-opacity: 0.1');
required.push('--md-sys-state-pressed-state-layer-opacity: 0.1');
required.push('--md-sys-state-dragged-state-layer-opacity: 0.16');
for (const token of required) {
  if (!tokensCss.includes(token)) {
    console.error(`check-tokens: tokens.css is missing required token: ${token}`);
    failures += 1;
  }
}

if (failures > 0) {
  console.error(`check-tokens: ${failures} violation(s). Use tokens.css variables instead.`);
  process.exit(1);
}
console.log('check-tokens: clean');
