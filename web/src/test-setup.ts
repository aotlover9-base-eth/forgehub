import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { afterAll, afterEach, beforeAll } from 'vitest';
import { server } from './test/msw';

beforeAll(() => server.listen({ onUnhandledRequest: 'error' }));
afterEach(() => {
  // vitest globals are off, so Testing Library's automatic
  // cleanup does not hook in; do it explicitly or renders leak
  // into the next test's document.
  cleanup();
  server.resetHandlers();
});
afterAll(() => server.close());
