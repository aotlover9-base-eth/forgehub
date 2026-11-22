import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor as waitForImpl } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { http, HttpResponse } from 'msw';
import { describe, expect, it } from 'vitest';
import axe from 'axe-core';
import { ToastProvider } from '../design';
import { authedHandlers, server, snippetFixture } from '../test/msw';
import SnippetPage from './SnippetPage';

function waitFor(fn: () => void) {
  return waitForImpl(fn, { timeout: 4000 });
}

function renderSnippet(path: string) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter(
    [
      { path: '/:owner/:name', element: <SnippetPage /> },
      { path: '/explore', element: <p>EXPLORE</p> },
      { path: '/dashboard', element: <p>DASHBOARD</p> },
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

describe('SnippetPage', () => {
  it("shows another user's public snippet with the safety banner and fg owner/name", async () => {
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/snippets/bob/tool', () =>
        HttpResponse.json(
          snippetFixture({ owner: 'bob', name: 'tool', visibility: 'public', body: 'echo hi', star_count: 4 }),
        ),
      ),
    );
    renderSnippet('/bob/tool');
    await waitFor(() => expect(screen.getByText(/Review commands before you run them/)).toBeTruthy());
    expect(screen.getByText('fg bob/tool')).toBeTruthy();
    expect(screen.getByText('Public')).toBeTruthy();
  });

  it('renders hostile content as inert text', async () => {
    const hostile = '<script>alert(1)</script>\n\u001b[31mred\u001b[0m\n\u202eREVERSED';
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/snippets/bob/evil', () =>
        HttpResponse.json(snippetFixture({ owner: 'bob', name: 'evil', visibility: 'public', body: hostile })),
      ),
    );
    const { container } = renderSnippet('/bob/evil');
    await waitFor(() => expect(screen.getByText(/alert\(1\)/)).toBeTruthy());
    // No script element was created from the body, and the
    // raw markup is visible as text.
    expect(container.querySelector('script')).toBeNull();
    expect(container.textContent).toContain('<script>alert(1)</script>');
    expect(container.textContent).toContain('\u202e');
  });

  it('shows the shared not-found page for a private snippet (no reveal)', async () => {
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/snippets/bob/secret', () =>
        HttpResponse.json({ error: { code: 'snippet_not_found', message: 'no' } }, { status: 404 }),
      ),
    );
    renderSnippet('/bob/secret');
    await waitFor(() => expect(screen.getByText('Not found')).toBeTruthy());
    expect(screen.getByText(/does not exist, or it is private/)).toBeTruthy();
  });

  it('stars another user\u2019s snippet', async () => {
    let starred = false;
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/snippets/bob/tool', () =>
        HttpResponse.json(snippetFixture({ owner: 'bob', name: 'tool', visibility: 'public', starred_by_me: starred, star_count: starred ? 5 : 4 })),
      ),
      http.put('/api/v1/snippets/bob/tool/star', () => {
        starred = true;
        return new HttpResponse(null, { status: 204 });
      }),
    );
    renderSnippet('/bob/tool');
    await waitFor(() => expect(screen.getByText('Star')).toBeTruthy());
    fireEvent.click(screen.getByText('Star'));
    await waitFor(() => expect(screen.getByText('Starred')).toBeTruthy());
  });

  it('compares two versions in the diff view', async () => {
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/snippets/ada/ubuntu-setup', () => HttpResponse.json(snippetFixture())),
      http.get('/api/v1/snippets/ada/ubuntu-setup/versions', () =>
        HttpResponse.json({
          versions: [
            { version: 2, hash: 'b'.repeat(64), short_hash: 'bbbbbbbb', message: 'second', created_at: '2026-01-02T00:00:00Z' },
            { version: 1, hash: 'a'.repeat(64), short_hash: 'aaaaaaaa', message: null, created_at: '2026-01-01T00:00:00Z' },
          ],
        }),
      ),
      http.get('/api/v1/snippets/ada/ubuntu-setup/diff', () =>
        HttpResponse.json({ from: 1, to: 2, diff: '@@ -1 +1 @@\n-echo old\n+echo new\n context' }),
      ),
    );
    const { container } = renderSnippet('/ada/ubuntu-setup');
    await waitFor(() => expect(screen.getByText('fg ubuntu-setup')).toBeTruthy());
    // Switch to the Versions tab.
    const tabs = document.querySelector('md-tabs');
    (tabs as unknown as { activeTabIndex: number }).activeTabIndex = 1;
    fireEvent.change(tabs!);
    // Tabs component reads activeTabIndex from the change event target.
    await waitFor(() => expect(screen.getByText(/v2 · bbbbbbbb/)).toBeTruthy());
    const boxes = container.querySelectorAll('md-checkbox');
    fireEvent.change(boxes[0]);
    fireEvent.change(boxes[1]);
    fireEvent.click(screen.getByText('Compare selected'));
    await waitFor(() => expect(screen.getByText('echo new')).toBeTruthy());
    expect(container.querySelector('.fh-diff__row--add')?.textContent).toContain('echo new');
    expect(container.querySelector('.fh-diff__row--remove')?.textContent).toContain('echo old');
  });

  it('rolls back to an old version by publishing it as a new version', async () => {
    const put: { ifMatch: string | null; body: { body?: string; message?: string } | null } = {
      ifMatch: null,
      body: null,
    };
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/snippets/ada/ubuntu-setup', () => HttpResponse.json(snippetFixture())),
      http.get('/api/v1/snippets/ada/ubuntu-setup/versions', () =>
        HttpResponse.json({
          versions: [
            { version: 3, hash: 'c'.repeat(64), short_hash: 'cccccccc', message: 'third', created_at: '2026-01-03T00:00:00Z' },
            { version: 1, hash: 'a'.repeat(64), short_hash: 'aaaaaaaa', message: 'first', created_at: '2026-01-01T00:00:00Z' },
          ],
        }),
      ),
      http.get('/api/v1/snippets/ada/ubuntu-setup/versions/1', () =>
        HttpResponse.json(snippetFixture({ version: 1, body: 'name: ubuntu-setup\n# v1 body' })),
      ),
      http.put('/api/v1/snippets/ada/ubuntu-setup', async ({ request }) => {
        put.ifMatch = request.headers.get('if-match');
        put.body = (await request.json()) as { body?: string; message?: string };
        return HttpResponse.json(snippetFixture({ version: 4 }));
      }),
    );
    renderSnippet('/ada/ubuntu-setup');
    await waitFor(() => expect(screen.getByText('fg ubuntu-setup')).toBeTruthy());
    const tabs = document.querySelector('md-tabs');
    (tabs as unknown as { activeTabIndex: number }).activeTabIndex = 1;
    fireEvent.change(tabs!);
    // Wait for the version rows; the only row-level Roll back
    // button belongs to v1 (v3 is current). The dialog's own
    // confirm button shares the label, so pick by ancestry.
    await waitFor(() => expect(screen.getByText(/v1 · aaaaaaaa/)).toBeTruthy());
    const rowButton = screen
      .getAllByText('Roll back')
      .find((b) => !b.closest('md-dialog'));
    if (!rowButton) throw new Error('row Roll back button not found');
    fireEvent.click(rowButton);
    const dialog = Array.from(document.querySelectorAll('md-dialog')).find((d) =>
      d.textContent?.includes('Roll back to v1'),
    );
    if (!dialog) throw new Error('rollback dialog not found');
    const confirm = Array.from(dialog.querySelectorAll('md-text-button')).find(
      (b) => b.textContent?.trim() === 'Roll back',
    );
    if (!confirm) throw new Error('confirm button not found');
    fireEvent.click(confirm);
    await waitFor(() => expect(put.body?.message).toBe('Rollback to v1'));
    expect(put.ifMatch).toBe('3');
    expect(put.body?.body).toContain('# v1 body');
  });

  it('has no axe violations', async () => {
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/snippets/bob/tool', () =>
        HttpResponse.json(snippetFixture({ owner: 'bob', name: 'tool', visibility: 'public', body: 'echo hi' })),
      ),
    );
    const { container } = renderSnippet('/bob/tool');
    await waitFor(() => expect(screen.getByText('fg bob/tool')).toBeTruthy());
    const results = await axe.run(container);
    expect(results.violations).toEqual([]);
  });
});
