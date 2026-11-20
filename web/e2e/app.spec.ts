import { expect, test } from '@playwright/test';

/**
 * The full web story in a real browser (Step 20):
 * signup -> home -> dashboard (screenshots at 3 widths) -> create
 * ubuntu-setup -> view -> edit -> versions + diff -> settings
 * token reveal-once flow -> device page error state.
 *
 * Selectors are role-based: Material web components expose
 * their controls in the accessibility tree, which Playwright
 * reads through shadow DOM.
 */

test('web story: signup, create, edit, diff, token, device', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  const button = (name: string | RegExp) => page.getByRole('button', { name });

  // --- signup -------------------------------------------------
  await page.goto('/signup');
  await page.getByRole('textbox', { name: 'Username' }).fill('webuser');
  await page.getByRole('textbox', { name: 'Email' }).fill('webuser@example.com');
  await page.getByRole('textbox', { name: 'Password' }).fill('a long passphrase');
  await button('Create account').click();
  // Since the Home separation (2026-11-20) signup lands on the
  // in-app Home page, not the Library.
  await expect(page).toHaveURL(/\/home$/);

  // --- dashboard screenshots at the three width classes -------
  for (const [name, width] of [
    ['compact', 400],
    ['medium', 800],
    ['expanded', 1280],
  ] as const) {
    await page.setViewportSize({ width, height: 900 });
    await page.goto('/dashboard');
    // Width-class captures are run artifacts, not docs: the
    // old dashboard-*.png files were removed from
    // docs/screenshots on purpose (2026-11-20).
    await page.screenshot({ path: `test-results/dashboard-${name}.png` });
  }
  await page.setViewportSize({ width: 1280, height: 900 });

  // --- create ubuntu-setup ------------------------------------
  await button('New snippet').first().click();
  await expect(page).toHaveURL(/\/new$/);
  await page.getByRole('textbox', { name: 'Name' }).fill('ubuntu-setup');
  await page.getByRole('textbox', { name: 'Description' }).fill('Browser E2E demo');
  await page.locator('label', { hasText: 'Public' }).first().click();
  await page.locator('textarea[aria-label="Script body"]').fill('echo browser-v1');
  await button('Save snippet').click();
  await expect(page).toHaveURL(/\/webuser\/ubuntu-setup$/);
  await expect(page.getByText('echo browser-v1')).toBeVisible();
  await expect(page.getByText('fg ubuntu-setup', { exact: true })).toBeVisible();

  // --- edit -> v2 ----------------------------------------------
  await button('Edit').first().click();
  await expect(page).toHaveURL(/\/edit$/);
  await page.locator('textarea[aria-label="Script body"]').fill('echo browser-v2');
  await button('Save new version').click();
  await expect(page).toHaveURL(/\/webuser\/ubuntu-setup$/);
  await expect(page.getByText('echo browser-v2')).toBeVisible();

  // --- versions + diff -----------------------------------------
  await page.getByText('Versions', { exact: true }).click();
  const boxes = page.getByRole('checkbox');
  await expect(boxes).toHaveCount(2);
  await boxes.nth(0).click();
  await boxes.nth(1).click();
  await button('Compare selected').click();
  await expect(page.getByText('browser-v2')).toBeVisible();
  await expect(page.getByText('browser-v1')).toBeVisible();

  // --- settings: token reveal-once ------------------------------
  await page.goto('/settings');
  await page.getByText('Tokens', { exact: true }).first().click();
  await button('New token').first().click();
  await page.getByRole('textbox', { name: 'Name' }).fill('browser-e2e');
  await button('Create token').last().click();
  await expect(page.getByText(/fgh_/).first()).toBeVisible();
  await button('Copy token').click();
  await button("I've copied it").click();

  // --- device page: unknown code is a clean error ---------------
  await page.goto('/device');
  await page.getByRole('textbox', { name: 'Device code' }).fill('ZZZZ-9999');
  await button('Continue').click();
  await expect(page.getByText(/not found|expired|invalid/i).first()).toBeVisible();
});
