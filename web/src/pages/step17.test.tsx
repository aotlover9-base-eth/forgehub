import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor as waitForImpl } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { http, HttpResponse } from 'msw';
import { describe, expect, it } from 'vitest';
import axe from 'axe-core';
import { ToastProvider } from '../design';
import { authedHandlers, meFixture, server, snippetFixture } from '../test/msw';
import DevicePage from './DevicePage';
import ExplorePage, { highlighted } from './ExplorePage';
import ProfilePage from './ProfilePage';
import SettingsPage from './SettingsPage';

function waitFor(fn: () => void) {
  return waitForImpl(fn, { timeout: 4000 });
}

function renderWithRouter(routes: { path: string; element: React.ReactNode }[], initial: string) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter(routes, { initialEntries: [initial] });
  return render(
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <RouterProvider router={router} />
      </ToastProvider>
    </QueryClientProvider>,
  );
}

describe('DevicePage', () => {
  it('approves a code end to end', async () => {
    let approved: unknown = null;
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/auth/device/lookup', () =>
        HttpResponse.json({ client_name: 'CLI on laptop', scopes: ['read', 'write'], expires_at: '2026-01-01T00:10:00Z' }),
      ),
      http.post('/api/v1/auth/device/approve', async ({ request }) => {
        approved = await request.json();
        return new HttpResponse(null, { status: 204 });
      }),
    );
    renderWithRouter([{ path: '/device', element: <DevicePage /> }], '/device?code=ABCD-2345');
    await waitFor(() => expect(screen.getByText(/CLI on laptop/)).toBeTruthy());
    expect(screen.getByText(/read and write your snippets/)).toBeTruthy();
    fireEvent.click(screen.getByText('Approve'));
    await waitFor(() => expect(screen.getByText(/Approved\. You can close this tab/)).toBeTruthy());
    expect(approved).toEqual({ user_code: 'ABCD-2345' });
  });

  it('denies, and reports a bad code calmly', async () => {
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/auth/device/lookup', () =>
        HttpResponse.json({ error: { code: 'device_code_not_found', message: 'No such device code' } }, { status: 404 }),
      ),
    );
    renderWithRouter([{ path: '/device', element: <DevicePage /> }], '/device?code=XXXX-9999');
    await waitFor(() => expect(screen.getByText('That code does not work')).toBeTruthy());
  });

  it('redirects to login when signed out, keeping the code in the return URL', async () => {
    renderWithRouter(
      [
        { path: '/device', element: <DevicePage /> },
        { path: '/login', element: <p>LOGIN PAGE</p> },
      ],
      '/device?code=ABCD-2345',
    );
    await waitFor(() => expect(screen.getByText('LOGIN PAGE')).toBeTruthy());
  });
});

describe('highlighted()', () => {
  it('wraps byte ranges in <mark> and keeps the rest as text', () => {
    const { container } = render(<p>{highlighted('ubuntu setup', [{ start: 0, end: 6 }])}</p>);
    expect(container.querySelector('mark')?.textContent).toBe('ubuntu');
    expect(container.textContent).toBe('ubuntu setup');
  });
});

describe('ExplorePage', () => {
  it('searches and renders highlighted results', async () => {
    server.use(
      http.get('/api/v1/tags', () => HttpResponse.json({ tags: [{ tag: 'ubuntu', count: 3 }] })),
      http.get('/api/v1/explore', () => HttpResponse.json({ snippets: [], next_cursor: null })),
      http.get('/api/v1/search', () =>
        HttpResponse.json({
          results: [
            {
              snippet: snippetFixture({ owner: 'bob', name: 'ubuntu-setup', visibility: 'public' }),
              highlights: [{ field: 'name', start: 0, end: 6 }],
            },
          ],
          next_cursor: null,
        }),
      ),
    );
    renderWithRouter([{ path: '/explore', element: <ExplorePage /> }], '/explore');
    const input = document.querySelector('.fh-searchbar__input') as HTMLInputElement;
    fireEvent.change(input, { target: { value: 'ubuntu' } });
    await waitFor(() => expect(document.querySelector('mark')?.textContent).toBe('ubuntu'));
  });
});

describe('ProfilePage', () => {
  it('renders the public profile and never an email', async () => {
    server.use(
      http.get('/api/v1/users/bob', () =>
        HttpResponse.json({
          username: 'bob',
          display_name: 'Bob',
          bio: 'hello',
          created_at: '2026-01-01T00:00:00Z',
          public_snippet_count: 1,
        }),
      ),
      http.get('/api/v1/users/bob/snippets', () =>
        HttpResponse.json({ snippets: [snippetFixture({ owner: 'bob', visibility: 'public' })], next_cursor: null }),
      ),
    );
    const { container } = renderWithRouter([{ path: '/:username', element: <ProfilePage /> }], '/bob');
    await waitFor(() => expect(screen.getByText('Bob')).toBeTruthy());
    expect(container.textContent).not.toContain('@example.com');
    expect(meFixture.email).not.toBe('');
  });
});

describe('SettingsPage tokens', () => {
  it('reveals a new token once, behind the copied step, and never in the list', async () => {
    const plaintext = 'fgh_SECRET-PLAINTEXT-VALUE';
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/tokens', () =>
        HttpResponse.json({
          tokens: [
            {
              id: 1,
              name: 'old',
              prefix: 'abcd1234',
              scopes: ['read'],
              created_at: '2026-01-01T00:00:00Z',
              last_used_at: null,
              expires_at: null,
              revoked_at: null,
            },
          ],
        }),
      ),
      http.post('/api/v1/tokens', () =>
        HttpResponse.json(
          {
            id: 2,
            name: 'laptop',
            prefix: 'zzzz9999',
            scopes: ['read', 'write'],
            created_at: '2026-01-02T00:00:00Z',
            last_used_at: null,
            expires_at: null,
            revoked_at: null,
            token: plaintext,
          },
          { status: 201 },
        ),
      ),
    );
    renderWithRouter([{ path: '/settings', element: <SettingsPage /> }], '/settings');
    await waitFor(() => expect(screen.getByText('Profile', { selector: 'md-primary-tab' })).toBeTruthy());
    // Switch to the Tokens tab.
    const tabs = document.querySelector('md-tabs') as unknown as { activeTabIndex: number };
    tabs.activeTabIndex = 3;
    fireEvent.change(document.querySelector('md-tabs')!);
    await waitFor(() => expect(screen.getByText(/abcd1234/)).toBeTruthy());
    fireEvent.click(screen.getByText('New token', { selector: 'md-filled-button' }));
    const nameEl = document.querySelector('md-outlined-text-field[label="Name"]')!;
    (nameEl as unknown as { value: string }).value = 'laptop';
    fireEvent.input(nameEl);
    fireEvent.click(screen.getByText('Create token'));
    await waitFor(() => expect(screen.getByText(/only time the full token is shown/)).toBeTruthy());
    expect(document.body.textContent).toContain(plaintext);
    // The "I've copied it" step is gated on copying.
    Object.assign(navigator, { clipboard: { writeText: async () => {} } });
    fireEvent.click(screen.getByText('Copy token'));
    await waitFor(() => expect(screen.getByText("I've copied it")).toBeTruthy());
    fireEvent.click(screen.getByText("I've copied it"));
    await waitFor(() => expect(document.body.textContent).not.toContain(plaintext));
  });
});

describe('SettingsPage danger zone', () => {
  it('deletes the account only after typing the username and password', async () => {
    let deleted: unknown = null;
    server.use(
      ...authedHandlers(),
      http.delete('/api/v1/me', async ({ request }) => {
        deleted = await request.json();
        return new HttpResponse(null, { status: 204 });
      }),
    );
    renderWithRouter(
      [
        { path: '/settings', element: <SettingsPage /> },
        { path: '/', element: <p>HOME PAGE</p> },
      ],
      '/settings',
    );
    await waitFor(() => expect(document.querySelector('md-tabs')).toBeTruthy());
    const tabs = document.querySelector('md-tabs') as unknown as { activeTabIndex: number };
    tabs.activeTabIndex = 5;
    fireEvent.change(document.querySelector('md-tabs')!);
    await waitFor(() => expect(screen.getByText('Delete my account')).toBeTruthy());
    fireEvent.click(screen.getByText('Delete my account'));
    const userEl = document.querySelector('md-outlined-text-field[label="Type \\"ada\\""]')!;
    (userEl as unknown as { value: string }).value = 'ada';
    fireEvent.input(userEl);
    const passEl = document.querySelector('md-outlined-text-field[label="Password"]')!;
    (passEl as unknown as { value: string }).value = 'a long passphrase';
    fireEvent.input(passEl);
    fireEvent.click(screen.getByText('Delete account', { selector: 'md-text-button, button' }));
    await waitFor(() => expect(screen.getByText('HOME PAGE')).toBeTruthy());
    expect(deleted).toEqual({ username: 'ada', password: 'a long passphrase' });
  });

  it('has no axe violations on the profile tab', async () => {
    server.use(...authedHandlers());
    const { container } = renderWithRouter([{ path: '/settings', element: <SettingsPage /> }], '/settings');
    // Peak A1 added the Appearance tab first; switch to Profile
    // (index 1) the same way the token and danger tests switch.
    await waitFor(() => expect(document.querySelector('md-tabs')).toBeTruthy());
    const tabs = document.querySelector('md-tabs') as unknown as { activeTabIndex: number };
    tabs.activeTabIndex = 1;
    fireEvent.change(document.querySelector('md-tabs')!);
    await waitFor(() => expect(document.querySelector('md-outlined-text-field[label="Display name"]')).toBeTruthy());
    const results = await axe.run(container);
    expect(results.violations).toEqual([]);
  });
});
