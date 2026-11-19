import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      '/api': 'http://127.0.0.1:8080',
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test-setup.ts'],
    globals: false,
    // Unit/component tests live under src/. The Playwright
    // suite in e2e/ runs under its own runner (Step 20); its
    // *.spec.ts name matches vitest's default include and
    // must not be collected here.
    include: ['src/**/*.{test,spec}.?(c|m)[jt]s?(x)'],
  },
});
