import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor as waitForImpl } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { http, HttpResponse } from 'msw';
import { describe, expect, it } from 'vitest';
import { ToastProvider } from '../design';
import { authedHandlers, meFixture, server } from '../test/msw';
import CollectionPage from './CollectionPage';
import CollectionsPanel from './CollectionsPanel';

function waitFor(fn: () => void) {
  return waitForImpl(fn, { timeout: 4000 });
}

const detail = {
  owner: 'anna',
  name: 'runbook',
  description: 'Ops runbook',
  visibility: 'public',
  item_count: 2,
  created_at: '2026-11-22T00:00:00Z',
  updated_at: '2026-11-22T00:00:00Z',
  items: [
    {
      owner: 'anna',
      name: 'restart-api',
      description: 'Restart the API',
      visibility: 'public',
      latest_version: 3,
      position: 0,
    },
    {
      owner: 'bob',
      name: 'check-disk',
      description: '',
      visibility: 'public',
      latest_version: 1,
      position: 1,
    },
  ],
};

function renderAt(path: string, element: React.ReactNode, routePath: string) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter(
    [
      { path: routePath, element },
      { path: '/dashboard', element: <p>DASHBOARD</p> },
      { path: '/collections/:owner/:name', element: <p>COLLECTION</p> },
      { path: '/:owner/:name', element: <p>SNIPPET</p> },
    ],
    { initialEntries: [path] },
  );
  return render(
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <RouterProvider router={router} />
      </ToastProvider>
    </QueryClientProvider>,
  );
}

describe('CollectionPage', () => {
  it('shows items in order with the run command, read-only for a non-owner view of a public collection', async () => {
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/collections/anna/runbook', () => HttpResponse.json(detail)),
    );
    // meFixture is a different user, so this is the visitor view.
    renderAt('/collections/anna/runbook', <CollectionPage />, '/collections/:owner/:name');
    await waitFor(() => expect(screen.getByText('anna/restart-api')).toBeTruthy());
    expect(screen.getByText('bob/check-disk')).toBeTruthy();
    expect(screen.getByText('fg run anna/runbook')).toBeTruthy();
    // No owner controls for a visitor.
    expect(screen.queryByText('Delete collection')).toBeNull();
    expect(screen.queryByLabelText(/Move restart-api up/)).toBeNull();
  });

  it('shows owner controls and the hidden-items note for the owner', async () => {
    server.use(
      http.get('/api/v1/me', () => HttpResponse.json({ ...meFixture, username: 'anna' })),
      http.get('/api/v1/collections/anna/runbook', () =>
        HttpResponse.json({ ...detail, total_items: 3 }),
      ),
    );
    renderAt('/collections/anna/runbook', <CollectionPage />, '/collections/:owner/:name');
    await waitFor(() => expect(screen.getByText('Delete collection')).toBeTruthy());
    expect(screen.getByText(/1 more item is not readable by you and hidden/)).toBeTruthy();
    expect(screen.getByLabelText(/Move restart-api down/)).toBeTruthy();
  });

  it('reorders through the API when the owner moves an item down', async () => {
    let posted: string[] | null = null;
    server.use(
      http.get('/api/v1/me', () => HttpResponse.json({ ...meFixture, username: 'anna' })),
      http.get('/api/v1/collections/anna/runbook', () => HttpResponse.json(detail)),
      http.post('/api/v1/collections/anna/runbook/reorder', async ({ request }) => {
        const body = (await request.json()) as { items: string[] };
        posted = body.items;
        return HttpResponse.json({ ...detail, item_count: 2 });
      }),
    );
    renderAt('/collections/anna/runbook', <CollectionPage />, '/collections/:owner/:name');
    await waitFor(() => {
      expect(screen.getByLabelText(/Move restart-api down/)).toBeTruthy();
    });
    fireEvent.click(screen.getByLabelText(/Move restart-api down/));
    await waitFor(() => expect(posted).toEqual(['bob/check-disk', 'anna/restart-api']));
  });

  it('renders the not-found state for a private or missing collection', async () => {
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/collections/anna/runbook', () =>
        HttpResponse.json(
          { error: { code: 'collection_not_found', message: 'Collection not found' } },
          { status: 404 },
        ),
      ),
    );
    renderAt('/collections/anna/runbook', <CollectionPage />, '/collections/:owner/:name');
    await waitFor(() => expect(screen.getByText('Collection not found')).toBeTruthy());
  });
});

describe('CollectionsPanel', () => {
  it('lists collections and creates a new one', async () => {
    let created: { name: string } | null = null;
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/me/collections', () =>
        HttpResponse.json({
          collections: [
            {
              owner: 'anna',
              name: 'runbook',
              description: 'Ops',
              visibility: 'public',
              item_count: 2,
              created_at: '2026-11-22T00:00:00Z',
              updated_at: '2026-11-22T00:00:00Z',
            },
          ],
        }),
      ),
      http.post('/api/v1/collections', async ({ request }) => {
        created = (await request.json()) as { name: string };
        return HttpResponse.json(
          {
            owner: 'anna',
            name: created.name,
            description: '',
            visibility: 'private',
            item_count: 0,
            created_at: '2026-11-22T00:00:00Z',
            updated_at: '2026-11-22T00:00:00Z',
          },
          { status: 201 },
        );
      }),
    );
    renderAt('/dashboard', <CollectionsPanel />, '/dashboard');
    await waitFor(() => expect(screen.getByText(/runbook/)).toBeTruthy());
    const field = document.querySelector('md-outlined-text-field[label="New collection name"]');
    if (!field) throw new Error('name field not found');
    (field as unknown as { value: string }).value = 'deploys';
    fireEvent.input(field);
    fireEvent.click(screen.getByText('Create'));
    await waitFor(() => expect(created?.name).toBe('deploys'));
  });
});
