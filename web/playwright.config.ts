import { defineConfig } from '@playwright/test';

/**
 * Browser E2E (Step 20). The server is started by
 * scripts/e2e-web.sh, which exports E2E_BASE_URL; this config
 * only drives the browser.
 */
export default defineConfig({
  testDir: './e2e',
  timeout: 60_000,
  retries: 0,
  reporter: [['list']],
  use: {
    baseURL: process.env.E2E_BASE_URL ?? 'http://127.0.0.1:18131',
    trace: 'retain-on-failure',
  },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
});
