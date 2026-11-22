import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor as waitForImpl } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { http, HttpResponse } from 'msw';
import { describe, expect, it } from 'vitest';
import { ToastProvider } from '../design';
import { authedHandlers, server } from '../test/msw';
import ImportPanel from './ImportPanel';

function waitFor(fn: () => void) {
  return waitForImpl(fn, { timeout: 4000 });
}

function renderPanel() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter([{ path: '/dashboard', element: <ImportPanel /> }], {
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

function setField(label: string, value: string) {
  const el = document.querySelector(`md-outlined-text-field[label="${label}"]`);
  if (!el) throw new Error(`field not found: ${label}`);
  (el as unknown as { value: string }).value = value;
  fireEvent.input(el);
}

describe('ImportPanel', () => {
  it('imports a gist and reports created and skipped files', async () => {
    let postedUrl: string | null = null;
    server.use(
      ...authedHandlers(),
      http.post('/api/v1/imports/gist', async ({ request }) => {
        const body = (await request.json()) as { url: string };
        postedUrl = body.url;
        return HttpResponse.json({
          created: [{ name: 'deploy', file: 'deploy.sh' }],
          skipped: [{ file: 'leaky.sh', reason: 'The snippet appears to contain secrets' }],
        });
      }),
    );
    renderPanel();
    setField('Gist URL', 'https://gist.github.com/anna/0123abcd');
    fireEvent.click(screen.getByText('Import gist'));
    await waitFor(() => expect(screen.getByText(/Imported/)).toBeTruthy());
    expect(postedUrl).toBe('https://gist.github.com/anna/0123abcd');
    expect(screen.getByText(/Skipped leaky\.sh/)).toBeTruthy();
  });

  it('analyzes a history file and imports the picked candidates as steps snippets', async () => {
    const created: { name?: string; body?: string }[] = [];
    server.use(
      ...authedHandlers(),
      http.post('/api/v1/imports/history/analyze', () =>
        HttpResponse.json({
          parsed: 11,
          kept: 9,
          candidates: [
            {
              name: 'docker-compose',
              commands: ['docker compose build', 'docker compose up -d'],
              count: 3,
            },
          ],
        }),
      ),
      http.post('/api/v1/snippets', async ({ request }) => {
        const body = (await request.json()) as { name?: string; body?: string };
        created.push(body);
        return HttpResponse.json({ name: body.name }, { status: 201 });
      }),
    );
    const { container } = renderPanel();
    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    const file = new File(['docker compose build\n'], 'history.txt', { type: 'text/plain' });
    fireEvent.change(input, { target: { files: [file] } });
    await waitFor(() => expect(screen.getByText(/seen 3 times/)).toBeTruthy());
    expect(screen.getByText(/11 commands read, 9 kept/)).toBeTruthy();
    fireEvent.click(screen.getByText('Import selected'));
    await waitFor(() => expect(created).toHaveLength(1));
    expect(created[0].name).toBe('docker-compose');
    expect(created[0].body).toContain('- run: "docker compose build"');
    expect(created[0].body).toContain('- run: "docker compose up -d"');
  });
});
