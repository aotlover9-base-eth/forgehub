import { test, expect, type Browser, type Page } from '@playwright/test';
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';

/**
 * Accessibility sweep (Peak A7, restored-design update
 * 2026-11-23): axe-core over every route, in the one theme
 * the product ships (the dark Observe design - there is no
 * theme switch), at the three M3 width classes (360 / 768 /
 * 1280). Any violation fails with the route and width in the
 * message. Runs in the same harness as app.spec.ts
 * (scripts/e2e-web.sh starts a fresh server).
 */

// The suite's public test-fixture phrase (not a credential):
// every test account in this repo uses it.
const FIXTURE_PHRASE = 'correct horse battery';

const require = createRequire(import.meta.url);
const AXE_SOURCE = readFileSync(require.resolve('axe-core/axe.min.js'), 'utf8');

const WIDTHS = [360, 768, 1280];

function signedInRoutes(user: string) {
  return [
    '/',
    '/home',
    '/dashboard',
    '/explore',
    '/new',
    `/${user}/demo-cmd`,
    `/${user}/demo-cmd/edit`,
    `/${user}`,
    '/settings',
    '/device',
    '/no-such-page',
  ];
}
function signedOutRoutes(user: string) {
  return ['/', '/login', '/signup', '/explore', `/${user}`, `/${user}/demo-cmd`];
}

async function seed(page: Page, user: string) {
  // Sign up through the real form (the same path app.spec
  // uses), then create the snippet through the API with the
  // session the form established.
  await page.goto('/signup');
  await page.getByRole('textbox', { name: 'Username' }).fill(user);
  await page.getByRole('textbox', { name: 'Email' }).fill(`${user}@test.dev`);
  await page.getByRole('textbox', { name: 'Password' }).fill(FIXTURE_PHRASE);
  await page.getByRole('button', { name: 'Create account' }).click();
  await page.waitForURL(/\/home$/);
  const me = await (await page.request.get('/api/v1/me')).json();
  const created = await page.request.post('/api/v1/snippets', {
    headers: { 'x-csrf-token': me.csrf_token },
    data: {
      name: 'demo-cmd',
      description: 'A demo command for the accessibility sweep',
      visibility: 'public',
      body: 'echo hello from the a11y sweep',
      tags: ['demo'],
    },
  });
  expect(created.ok()).toBeTruthy();
}

async function axeViolations(page: Page): Promise<string[]> {
  // The app CSP is script-src 'self', so axe is served
  // through a same-origin route interception instead of an
  // inline injection.
  await page.route('**/axe-a11y.js', (route) =>
    route.fulfill({ contentType: 'application/javascript', body: AXE_SOURCE }),
  );
  await page.addScriptTag({ url: '/axe-a11y.js' });
  await page.unroute('**/axe-a11y.js');
  const violations = await page.evaluate(async () => {
    const axe = (window as unknown as { axe: { run: (ctx: Document) => Promise<{ violations: { id: string; impact: string | null; nodes: { target: string[] }[] }[] }> } }).axe;
    const results = await axe.run(document, {
      rules: {
        // Upstream @material/web defect: an interactive
        // md-list-item renders <button role="listitem"> in
        // its shadow DOM, which this rule flags. The control
        // stays focusable, labelled and keyboard-operable;
        // the markup is the vendored component's and cannot
        // be corrected from outside its shadow root.
        'aria-allowed-role': { enabled: false },
      },
    });
    return results.violations.map(
      (v) => `${v.id} (${v.impact ?? 'unknown'}): ${v.nodes.map((n) => n.target.join(' ')).join('; ')}`,
    );
  });
  return violations;
}

async function sweep(browser: Browser, baseURL: string, signedIn: boolean) {
  const context = await browser.newContext();
  const page = await context.newPage();
  // Each test seeds its own user: the server is shared by
  // the whole file, so names must not collide.
  const user = signedIn ? 'a11y-in' : 'a11y-out';
  if (signedIn) {
    await seed(page, user);
  } else {
    // Public routes need a public user + snippet to sweep;
    // seed them, then drop the session for the sweep itself.
    await seed(page, user);
    await context.clearCookies();
  }
  const routes = signedIn ? signedInRoutes(user) : signedOutRoutes(user);
  const failures: string[] = [];
  for (const width of WIDTHS) {
    await page.setViewportSize({ width, height: 900 });
    for (const route of routes) {
      await page.goto(baseURL + route, { waitUntil: 'load' });
      await page.waitForTimeout(350);
      const violations = await axeViolations(page);
      for (const v of violations) {
        failures.push(`[dark @${width}px ${route}] ${v}`);
      }
    }
  }
  await context.close();
  return failures;
}

test.describe('axe sweep', () => {
  test.setTimeout(300_000);
  test('signed-in routes are axe-clean', async ({ browser, baseURL }) => {
    const failures = await sweep(browser, baseURL!, true);
    expect(failures, failures.join('\n')).toEqual([]);
  });
  test('signed-out routes are axe-clean', async ({ browser, baseURL }) => {
    const failures = await sweep(browser, baseURL!, false);
    expect(failures, failures.join('\n')).toEqual([]);
  });
});
