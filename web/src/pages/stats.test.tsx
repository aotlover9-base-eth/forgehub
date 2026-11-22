import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor as waitForImpl } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { http, HttpResponse } from 'msw';
import { describe, expect, it } from 'vitest';
import { ToastProvider } from '../design';
import { authedHandlers, server } from '../test/msw';
import LibraryStats from './LibraryStats';

function waitFor(fn: () => void) {
  return waitForImpl(fn, { timeout: 4000 });
}

function renderStats() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter([{ path: '/dashboard', element: <LibraryStats /> }], {
    initialEntries: ['/dashboard'],
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <RouterProvider router={router} />
      </ToastProvider>
    </QueryClientProvider>,
  );
}

describe('LibraryStats (Peak C8)', () => {
  it('shows totals and the top workflows', async () => {
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/me/stats', () =>
        HttpResponse.json({
          runs: 3,
          commands: 8,
          total_ms: 2_800,
          failures: 1,
          estimated_saved_ms: 93_200,
          workflows: [
            {
              ref: 'anna/deploy',
              runs: 2,
              commands: 6,
              total_ms: 2_400,
              failures: 1,
              estimated_saved_ms: 69_600,
              last_run: '2026-11-22T01:02:03Z',
            },
            {
              ref: 'bob/setup',
              runs: 1,
              commands: 2,
              total_ms: 400,
              failures: 0,
              estimated_saved_ms: 23_600,
              last_run: '2026-11-21T01:02:03Z',
            },
          ],
        }),
      ),
    );
    renderStats();
    await waitFor(() => expect(screen.getByText('Your library')).toBeTruthy());
    expect(screen.getByText('cloud runs')).toBeTruthy();
    expect(screen.getByText('commands automated')).toBeTruthy();
    expect(screen.getByText('estimated time saved')).toBeTruthy();
    expect(screen.getByText('anna/deploy')).toBeTruthy();
    expect(screen.getByText(/2 runs - 1 failed - last run 2026-11-22/)).toBeTruthy();
    expect(screen.getByText('bob/setup')).toBeTruthy();
    expect(screen.getByText(/Counts only your own runs/)).toBeTruthy();
  });

  it('explains the empty state and where local runs live', async () => {
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/me/stats', () =>
        HttpResponse.json({
          runs: 0,
          commands: 0,
          total_ms: 0,
          failures: 0,
          estimated_saved_ms: 0,
          workflows: [],
        }),
      ),
    );
    renderStats();
    await waitFor(() => expect(screen.getByText(/No cloud runs yet/)).toBeTruthy());
    expect(screen.getByText(/fg stats/)).toBeTruthy();
  });
});
