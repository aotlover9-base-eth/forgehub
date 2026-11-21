import { test, expect, type Browser, type Page } from '@playwright/test';

/**
 * Visual regression (Peak A8): 12 key screens x 2 themes x
 * 3 width classes against goldens in __screenshots__.
 * Gated behind VISUAL_TESTS=1 (scripts/visual-web.sh sets
 * it) so the default e2e run stays fast.
 *
 * Determinism rules:
 * - The seed is fixed (one user, three snippets with fixed
 *   bodies and tags).
 * - Screens that render relative times get a browser clock
 *   frozen in the past, so every relative time clamps to
 *   "just now" in every run (see relativeTime in
 *   src/validation.ts).
 * - The profile "joined" line is data-driven (month/year of
 *   the seed's creation) and is masked.
 * - Animations are disabled and reduced motion is emulated,
 *   so A6 entrances sit at their end state.
 */

test.skip(process.env.VISUAL_TESTS !== '1', 'visual suite is opt-in (scripts/visual-web.sh)');

const FIXTURE_PHRASE = 'correct horse battery';
const WIDTHS = [360, 768, 1280];
const THEMES = ['light', 'dark'] as const;

async function seed(page: Page, user: string) {
  await page.goto('/signup');
  await page.getByRole('textbox', { name: 'Username' }).fill(user);
  await page.getByRole('textbox', { name: 'Email' }).fill(`${user}@test.dev`);
  await page.getByRole('textbox', { name: 'Password' }).fill(FIXTURE_PHRASE);
  await page.getByRole('button', { name: 'Create account' }).click();
  await page.waitForURL(/\/home$/);
  const me = await (await page.request.get('/api/v1/me')).json();
  const snippets = [
    {
      name: 'demo-cmd',
      description: 'A demo command for visual checks',
      visibility: 'public',
      body: 'echo hello from ForgeHub\necho save once, run anywhere',
      tags: ['demo', 'visual'],
    },
    {
      name: 'private-notes',
      description: 'A private snippet',
      visibility: 'private',
      body: 'echo secret plans',
      tags: ['notes'],
    },
    {
      name: 'shared-link',
      description: 'An unlisted snippet',
      visibility: 'unlisted',
      body: 'echo shared by link',
      tags: [],
    },
  ];
  for (const snippet of snippets) {
    const created = await page.request.post('/api/v1/snippets', {
      headers: { 'x-csrf-token': me.csrf_token },
      data: snippet,
    });
    expect(created.ok()).toBeTruthy();
  }
}

async function login(page: Page, user: string) {
  await page.goto('/login');
  await page.getByRole('textbox', { name: 'Email or username' }).fill(user);
  await page.getByRole('textbox', { name: 'Password' }).fill(FIXTURE_PHRASE);
  await page.getByRole('main').getByRole('button', { name: 'Log in' }).click();
  await page.waitForURL(/\/home$/);
}

async function shot(page: Page, name: string, mask: ReturnType<Page['locator']>[] = []) {
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(250);
  await expect(page).toHaveScreenshot(name, { animations: 'disabled', mask });
}

function frozenScreens(user: string): { name: string; path: string }[] {
  return [
    { name: 'home', path: '/home' },
    { name: 'library', path: '/dashboard' },
    { name: 'explore', path: '/explore' },
    { name: 'snippet', path: `/${user}/demo-cmd` },
    { name: 'profile', path: `/${user}` },
  ];
}
function liveScreens(user: string): { name: string; path: string }[] {
  return [
    { name: 'editor-new', path: '/new' },
    { name: 'editor-edit', path: `/${user}/demo-cmd/edit` },
    { name: 'settings', path: '/settings' },
    { name: 'device', path: '/device' },
    { name: 'not-found', path: '/no-such-page' },
  ];
}
const OUT_SCREENS: { name: string; path: string }[] = [
  { name: 'landing', path: '/' },
  { name: 'login', path: '/login' },
];

async function runSet(browser: Browser, baseURL: string, theme: string) {
  // One user per theme, created once (first frozen context);
  // every other context logs in, so Explore and the profile
  // show exactly the same data at every width.
  const user = `visual-${theme}`;
  let seeded = false;
  for (const width of WIDTHS) {
    // Signed-in, frozen clock (relative-time screens).
    const frozen = await browser.newContext({
      viewport: { width, height: 900 },
      colorScheme: theme as 'light' | 'dark',
      reducedMotion: 'reduce',
    });
    await frozen.addInitScript((t) => window.localStorage.setItem('forgehub-theme', t), theme);
    const frozenPage = await frozen.newPage();
    if (!seeded) {
      await seed(frozenPage, user);
      seeded = true;
    } else {
      await login(frozenPage, user);
    }
    await frozenPage.clock.setFixedTime(new Date('2020-01-01T00:00:00Z'));
    for (const screen of frozenScreens(user)) {
      await frozenPage.goto(baseURL + screen.path, { waitUntil: 'load' });
      const mask =
        screen.name === 'profile'
          ? [frozenPage.locator('p', { hasText: 'joined' })]
          : [];
      await shot(frozenPage, `${screen.name}-${theme}-${width}.png`, mask);
    }
    await frozen.close();

    // Signed-in, live clock (editor validation, static pages).
    const live = await browser.newContext({
      viewport: { width, height: 900 },
      colorScheme: theme as 'light' | 'dark',
      reducedMotion: 'reduce',
    });
    await live.addInitScript((t) => window.localStorage.setItem('forgehub-theme', t), theme);
    const livePage = await live.newPage();
    await login(livePage, user);
    for (const screen of liveScreens(user)) {
      await livePage.goto(baseURL + screen.path, { waitUntil: 'load' });
      if (screen.name.startsWith('editor')) {
        // Let the validation debounce settle.
        await livePage.waitForTimeout(900);
      }
      await shot(livePage, `${screen.name}-${theme}-${width}.png`);
    }
    await live.close();

    // Signed-out.
    const out = await browser.newContext({
      viewport: { width, height: 900 },
      colorScheme: theme as 'light' | 'dark',
      reducedMotion: 'reduce',
    });
    await out.addInitScript((t) => window.localStorage.setItem('forgehub-theme', t), theme);
    const outPage = await out.newPage();
    for (const screen of OUT_SCREENS) {
      await outPage.goto(baseURL + screen.path, { waitUntil: 'load' });
      await shot(outPage, `${screen.name}-${theme}-${width}.png`);
    }
    await out.close();
  }
}

test.describe('visual regression', () => {
  test.setTimeout(600_000);
  for (const theme of THEMES) {
    test(`12 screens x 3 widths in ${theme}`, async ({ browser, baseURL }) => {
      await runSet(browser, baseURL!, theme);
    });
  }
});
