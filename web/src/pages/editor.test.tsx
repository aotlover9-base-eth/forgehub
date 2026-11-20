import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor as waitForImpl } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { http, HttpResponse } from 'msw';
import { describe, expect, it } from 'vitest';
import { ToastProvider } from '../design';
import { authedHandlers, server, snippetFixture } from '../test/msw';
import EditorPage from './EditorPage';

function waitFor(fn: () => void) {
  return waitForImpl(fn, { timeout: 4000 });
}

function renderEditor(path: string) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter(
    [
      { path: '/new', element: <EditorPage /> },
      { path: '/:owner/:name/edit', element: <EditorPage /> },
      { path: '/:owner/:name', element: <p>DETAIL PAGE</p> },
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

function setField(label: string, value: string) {
  const el = document.querySelector(`md-outlined-text-field[label="${label}"]`);
  if (!el) throw new Error(`field not found: ${label}`);
  (el as unknown as { value: string }).value = value;
  fireEvent.input(el);
}
function bodyArea(): HTMLTextAreaElement {
  return screen.getByLabelText('Script body') as HTMLTextAreaElement;
}

const validResult = {
  valid: true,
  errors: [],
  hash: 'a'.repeat(64),
  summary: {
    mode: 'script',
    steps_count: 2,
    variables: ['region'],
    uses_sudo: true,
    lint_warnings: [{ kind: 'sudo', line: 1, message: 'Uses sudo' }],
  },
};

describe('EditorPage (new)', () => {
  it('shows the live validation panel: variables, sudo, warnings', async () => {
    server.use(
      ...authedHandlers(),
      http.post('/api/v1/snippets/validate', () => HttpResponse.json(validResult)),
      http.get('/api/v1/snippets/ada/ubuntu-setup', () =>
        HttpResponse.json({ error: { code: 'snippet_not_found', message: 'no' } }, { status: 404 }),
      ),
    );
    renderEditor('/new');
    setField('Name', 'ubuntu-setup');
    fireEvent.change(bodyArea(), { target: { value: 'sudo apt-get update\necho $region' } });
    await waitFor(() => expect(screen.getByText(/Valid - ready to save/)).toBeTruthy());
    expect(document.querySelector('md-assist-chip[label="region"]')).toBeTruthy();
    expect(screen.getAllByText(/Uses sudo/).length).toBeGreaterThanOrEqual(1);
  });

  it('blocks saving when the server finds secrets, without echoing the value', async () => {
    server.use(
      ...authedHandlers(),
      http.post('/api/v1/snippets/validate', () =>
        HttpResponse.json(
          {
            valid: false,
            errors: [],
            secret_findings: [{ kind: 'aws_access_key', line: 2 }],
          },
          { status: 422 },
        ),
      ),
      http.get('/api/v1/snippets/ada/ubuntu-setup', () =>
        HttpResponse.json({ error: { code: 'snippet_not_found', message: 'no' } }, { status: 404 }),
      ),
    );
    renderEditor('/new');
    setField('Name', 'ubuntu-setup');
    fireEvent.change(bodyArea(), { target: { value: 'echo ok\nexport KEY=AKIAEXAMPLEVALUE' } });
    await waitFor(() => expect(screen.getByText(/Secrets found - saving is blocked/)).toBeTruthy());
    expect(screen.getByText(/Line 2: possible aws access key/)).toBeTruthy();
    const save = screen.getByText('Save snippet').closest('md-filled-button') as HTMLElement & { disabled?: boolean };
    expect(save.disabled === true || save.hasAttribute('disabled')).toBe(true);
    // The findings UI shows kind + line only; the value the
    // user typed lives in the textarea (their own input) but
    // must never be echoed back by the checks panel.
    expect(screen.getByRole('alert').textContent).not.toContain('AKIAEXAMPLEVALUE');
  });

  it('creates the snippet and navigates to its page', async () => {
    let posted: unknown = null;
    server.use(
      ...authedHandlers(),
      http.post('/api/v1/snippets/validate', () => HttpResponse.json(validResult)),
      http.get('/api/v1/snippets/ada/ubuntu-setup', () =>
        HttpResponse.json({ error: { code: 'snippet_not_found', message: 'no' } }, { status: 404 }),
      ),
      http.post('/api/v1/snippets', async ({ request }) => {
        posted = await request.json();
        return HttpResponse.json(snippetFixture(), { status: 201 });
      }),
    );
    renderEditor('/new');
    setField('Name', 'ubuntu-setup');
    fireEvent.change(bodyArea(), { target: { value: 'sudo apt-get update' } });
    await waitFor(() => expect(screen.getByText(/Valid - ready to save/)).toBeTruthy());
    fireEvent.click(screen.getByText('Save snippet'));
    await waitFor(() => expect(screen.getByText('DETAIL PAGE')).toBeTruthy());
    expect((posted as { name: string }).name).toBe('ubuntu-setup');
  });

  it('Tab inserts spaces; Esc then Tab does not (no keyboard trap)', async () => {
    server.use(
      ...authedHandlers(),
      http.post('/api/v1/snippets/validate', () => HttpResponse.json(validResult)),
    );
    renderEditor('/new');
    const area = bodyArea();
    fireEvent.change(area, { target: { value: 'echo hi' } });
    area.setSelectionRange(4, 4);
    fireEvent.keyDown(area, { key: 'Tab' });
    expect(area.value).toBe('echo   hi');
    fireEvent.keyDown(area, { key: 'Escape' });
    const before = area.value;
    fireEvent.keyDown(area, { key: 'Tab' });
    expect(area.value).toBe(before);
  });
});

describe('EditorPage (edit, optimistic concurrency)', () => {
  it('offers the conflict dialog on 412 and overwrites with the fresh version', async () => {
    const v3 = snippetFixture({ version: 3, body: 'echo v3' });
    const v4 = snippetFixture({ version: 4, body: 'echo v4-someone-else' });
    let getCount = 0;
    let lastIfMatch: string | null = null;
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/snippets/ada/ubuntu-setup', () => {
        getCount += 1;
        return HttpResponse.json(getCount === 1 ? v3 : v4);
      }),
      http.post('/api/v1/snippets/validate', () => HttpResponse.json(validResult)),
      http.put('/api/v1/snippets/ada/ubuntu-setup', ({ request }) => {
        lastIfMatch = request.headers.get('If-Match');
        if (lastIfMatch === '3') {
          return HttpResponse.json(
            { error: { code: 'version_conflict', message: 'conflict' } },
            { status: 412 },
          );
        }
        return HttpResponse.json(snippetFixture({ version: 5, body: 'echo mine' }));
      }),
    );
    renderEditor('/ada/ubuntu-setup/edit');
    await waitFor(() => expect(bodyArea().value).toBe('echo v3'));
    fireEvent.change(bodyArea(), { target: { value: 'echo mine' } });
    await waitFor(() => expect(screen.getByText(/Valid - ready to save/)).toBeTruthy());
    fireEvent.click(screen.getByText('Save new version'));
    await waitFor(() => expect(screen.getByText('Someone saved a newer version')).toBeTruthy());
    fireEvent.click(screen.getByText('Overwrite with mine'));
    await waitFor(() => expect(screen.getByText('DETAIL PAGE')).toBeTruthy());
    expect(lastIfMatch).toBe('4');
  });
});
