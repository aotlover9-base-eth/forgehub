import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor as waitForImpl } from '@testing-library/react';

// Generous timeout: the suite runs files in parallel on a
// shared CPU, and the default 1 s is too tight there.
function waitFor(fn: () => void) {
  return waitForImpl(fn, { timeout: 4000 });
}
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { http, HttpResponse } from 'msw';
import { describe, expect, it } from 'vitest';
import axe from 'axe-core';
import { ToastProvider } from '../design';
import { authedHandlers, meFixture, server, snippetFixture } from '../test/msw';
import LoginPage from './LoginPage';
import SignupPage from './SignupPage';
import DashboardPage from './DashboardPage';
import HomePage from './HomePage';
import ForgotPasswordPage from './ForgotPasswordPage';
import ResetPasswordPage from './ResetPasswordPage';

function renderPage(ui: React.ReactNode, route = '/') {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <MemoryRouter initialEntries={[route]}>
          <Routes>
            <Route path="/login" element={<LoginPage />} />
            <Route path="/signup" element={<SignupPage />} />
            <Route path="/dashboard" element={<DashboardPage />} />
            <Route path="*" element={ui} />
          </Routes>
        </MemoryRouter>
      </ToastProvider>
    </QueryClientProvider>,
  );
}

function field(label: string): HTMLInputElement {
  // md-outlined-text-field carries the label attribute; the
  // editable element is the custom element itself in jsdom.
  const el = document.querySelector(`md-outlined-text-field[label="${label}"]`);
  if (!el) throw new Error(`field not found: ${label}`);
  return el as unknown as HTMLInputElement;
}
function setField(label: string, value: string) {
  const el = field(label);
  (el as unknown as { value: string }).value = value;
  fireEvent.input(el);
}

describe('LoginPage', () => {
  it('logs in and navigates to the return URL', async () => {
    server.use(
      http.post('/api/v1/auth/login', () => HttpResponse.json(meFixture)),
      http.get('/api/v1/me', () => HttpResponse.json(meFixture)),
      http.get('/api/v1/me/snippets', () => HttpResponse.json({ snippets: [], next_cursor: null })),
    );
    renderPage(<LoginPage />, '/login?return=/dashboard');
    setField('Email or username', 'ada');
    setField('Password', 'correct horse battery');
    fireEvent.submit(document.querySelector('form')!);
    await waitFor(() => expect(screen.getByText('Your snippets')).toBeTruthy());
  });

  it('shows the generic failure on 401 (no account oracle)', async () => {
    server.use(
      http.post('/api/v1/auth/login', () =>
        HttpResponse.json({ error: { code: 'invalid_credentials', message: 'Invalid credentials' } }, { status: 401 }),
      ),
    );
    renderPage(<LoginPage />, '/login');
    setField('Email or username', 'ada');
    setField('Password', 'wrong-password');
    fireEvent.submit(document.querySelector('form')!);
    await waitFor(() => expect(screen.getByRole('alert').textContent).toMatch(/did not match/));
  });

  it('shows the wait time on 429', async () => {
    server.use(
      http.post('/api/v1/auth/login', () =>
        HttpResponse.json(
          { error: { code: 'rate_limited', message: 'Slow down' } },
          { status: 429, headers: { 'Retry-After': '42' } },
        ),
      ),
    );
    renderPage(<LoginPage />, '/login');
    setField('Email or username', 'ada');
    setField('Password', 'wrong-password');
    fireEvent.submit(document.querySelector('form')!);
    await waitFor(() => expect(screen.getByRole('alert').textContent).toMatch(/42 seconds/));
  });

  it('has no axe violations', async () => {
    const { container } = renderPage(<LoginPage />, '/login');
    const results = await axe.run(container);
    expect(results.violations).toEqual([]);
  });
});

describe('SignupPage', () => {
  it('shows live username guidance matching the server rules', async () => {
    renderPage(<SignupPage />, '/signup');
    setField('Username', 'Bad Name');
    await waitFor(() =>
      expect(field('Username').getAttribute('error-text')).toMatch(/Lowercase letters/),
    );
    setField('Username', 'admin');
    await waitFor(() => expect(field('Username').getAttribute('error-text')).toMatch(/reserved/));
  });

  it('maps username_taken to the username field', async () => {
    server.use(
      http.post('/api/v1/auth/signup', () =>
        HttpResponse.json({ error: { code: 'username_taken', message: 'Username taken' } }, { status: 409 }),
      ),
    );
    renderPage(<SignupPage />, '/signup');
    setField('Username', 'ada');
    setField('Email', 'ada@example.com');
    setField('Password', 'a long passphrase');
    fireEvent.submit(document.querySelector('form')!);
    await waitFor(() => expect(field('Username').getAttribute('error-text')).toMatch(/taken/));
  });

  it('has no axe violations', async () => {
    const { container } = renderPage(<SignupPage />, '/signup');
    const results = await axe.run(container);
    expect(results.violations).toEqual([]);
  });
});

describe('DashboardPage', () => {
  it('lists snippets with visibility and version', async () => {
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/me/snippets', () =>
        HttpResponse.json({
          snippets: [snippetFixture(), snippetFixture({ name: 'docker-clean', visibility: 'public', version: 1 })],
          next_cursor: null,
        }),
      ),
    );
    renderPage(<DashboardPage />, '/dashboard');
    await waitFor(() => expect(screen.getByText('ubuntu-setup')).toBeTruthy());
    expect(screen.getByText('docker-clean')).toBeTruthy();
    expect(screen.getByText('Public')).toBeTruthy();
    expect(screen.getByText('Private')).toBeTruthy();
  });

  it('shows the empty state with the install card when there are none', async () => {
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/me/snippets', () => HttpResponse.json({ snippets: [], next_cursor: null })),
    );
    renderPage(<DashboardPage />, '/dashboard');
    await waitFor(() => expect(screen.getByText('No snippets yet')).toBeTruthy());
    expect(screen.getByText('Run them from your terminal')).toBeTruthy();
  });

  it('passes the visibility filter to the API', async () => {
    let seenVisibility: string | null = null;
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/me/snippets', ({ request }) => {
        seenVisibility = new URL(request.url).searchParams.get('visibility');
        return HttpResponse.json({ snippets: [], next_cursor: null });
      }),
    );
    renderPage(<DashboardPage />, '/dashboard');
    await waitFor(() => expect(screen.getByText('No snippets yet')).toBeTruthy());
    fireEvent.click(document.querySelector('md-filter-chip[label="Public"]')!);
    await waitFor(() => expect(seenVisibility).toBe('public'));
  });

  it('redirects to login when the session has expired (401)', async () => {
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/me/snippets', () =>
        HttpResponse.json({ error: { code: 'unauthorized', message: 'Login required' } }, { status: 401 }),
      ),
    );
    renderPage(<DashboardPage />, '/dashboard');
    await waitFor(() => expect(screen.getByText('Log in', { selector: 'h1' })).toBeTruthy());
  });

  it('has no axe violations with data', async () => {
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/me/snippets', () =>
        HttpResponse.json({ snippets: [snippetFixture()], next_cursor: null }),
      ),
    );
    const { container } = renderPage(<DashboardPage />, '/dashboard');
    await waitFor(() => expect(screen.getByText('ubuntu-setup')).toBeTruthy());
    const results = await axe.run(container);
    expect(results.violations).toEqual([]);
  });
});

describe('HomePage', () => {
  it('greets the user and shows recent + popular snippets', async () => {
    server.use(
      http.get('/api/v1/me', () => HttpResponse.json(meFixture)),
      http.get('/api/v1/me/snippets', () =>
        HttpResponse.json({ snippets: [snippetFixture()], next_cursor: null }),
      ),
      http.get('/api/v1/explore', () =>
        HttpResponse.json({
          snippets: [snippetFixture({ owner: 'bob', name: 'disk-usage', star_count: 7 })],
          next_cursor: null,
        }),
      ),
    );
    renderPage(<HomePage />, '/home');
    await waitFor(() => expect(screen.getByText('Welcome back, Ada')).toBeTruthy());
    expect(screen.getByText('Your recent snippets')).toBeTruthy();
    expect(screen.getByText('ubuntu-setup')).toBeTruthy();
    expect(screen.getByText('Popular this week')).toBeTruthy();
    expect(screen.getByText('disk-usage')).toBeTruthy();
    expect(screen.getByText('View all in your Library')).toBeTruthy();
  });

  it('offers a first-snippet call to action when the library is empty', async () => {
    server.use(
      http.get('/api/v1/me', () => HttpResponse.json(meFixture)),
      http.get('/api/v1/me/snippets', () => HttpResponse.json({ snippets: [], next_cursor: null })),
      http.get('/api/v1/explore', () => HttpResponse.json({ snippets: [], next_cursor: null })),
    );
    renderPage(<HomePage />, '/home');
    await waitFor(() => expect(screen.getByText('Save your first snippet')).toBeTruthy());
  });
});

describe('Password reset pages', () => {
  it('forgot: posts the email and lands on the enumeration-safe confirmation', async () => {
    let posted: unknown = null;
    server.use(
      http.post('/api/v1/auth/password-reset/request', async ({ request }) => {
        posted = await request.json();
        return new HttpResponse(null, { status: 204 });
      }),
    );
    renderPage(<ForgotPasswordPage />, '/forgot-password');
    setField('Email', 'ada@example.com');
    fireEvent.submit(document.querySelector('form')!);
    await waitFor(() =>
      expect(screen.getByText(/If an account exists for/)).toBeTruthy(),
    );
    expect(posted).toEqual({ email: 'ada@example.com' });
  });

  it('reset: matching passwords post token + new password and route to login', async () => {
    let posted: unknown = null;
    server.use(
      http.post('/api/v1/auth/password-reset/confirm', async ({ request }) => {
        posted = await request.json();
        return new HttpResponse(null, { status: 204 });
      }),
    );
    renderPage(<ResetPasswordPage />, '/reset-password?token=tok-123');
    setField('New password', 'a-brand-new-passphrase-42');
    setField('Confirm new password', 'a-brand-new-passphrase-42');
    fireEvent.submit(document.querySelector('form')!);
    await waitFor(() => expect(posted).not.toBeNull());
    expect(posted).toEqual({ token: 'tok-123', new_password: 'a-brand-new-passphrase-42' });
  });

  it('reset: mismatched passwords never reach the API', async () => {
    let called = false;
    server.use(
      http.post('/api/v1/auth/password-reset/confirm', () => {
        called = true;
        return new HttpResponse(null, { status: 204 });
      }),
    );
    renderPage(<ResetPasswordPage />, '/reset-password?token=tok-123');
    setField('New password', 'a-brand-new-passphrase-42');
    setField('Confirm new password', 'something-else-entirely-9');
    fireEvent.submit(document.querySelector('form')!);
    await waitFor(() =>
      expect(screen.getByText('The two passwords do not match.')).toBeTruthy(),
    );
    expect(called).toBe(false);
  });

  it('reset: a tokenless link explains itself instead of showing a form', async () => {
    renderPage(<ResetPasswordPage />, '/reset-password');
    await waitFor(() =>
      expect(screen.getByText(/missing its token/)).toBeTruthy(),
    );
    expect(document.querySelector('form')).toBeNull();
  });
});
