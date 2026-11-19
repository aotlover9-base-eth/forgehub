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

if (failures > 0) {
  console.error(`check-tokens: ${failures} violation(s). Use tokens.css variables instead.`);
  process.exit(1);
}
console.log('check-tokens: clean');
