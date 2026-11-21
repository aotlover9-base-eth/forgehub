import { fireEvent, render, screen } from '@testing-library/react';
import { http, HttpResponse } from 'msw';
import { describe, expect, it } from 'vitest';
import App from './App';
import { authedHandlers, server } from './test/msw';

// Signed-in shell navigation. This lives in its own file
// because App's router and query client are module-level:
// App.test.tsx caches /me as signed-out (null), and a fresh
// module graph is the clean way to start signed in.
describe('App navigation (signed in)', () => {
  it('Home stays Home and Profile highlights Profile', async () => {
    server.use(
      ...authedHandlers(),
      http.get('/api/v1/users/ada', () =>
        HttpResponse.json({
          username: 'ada',
          display_name: 'Ada',
          bio: null,
          created_at: '2026-01-01T00:00:00Z',
          public_snippet_count: 0,
        }),
      ),
      http.get('/api/v1/users/ada/snippets', () =>
        HttpResponse.json({ snippets: [], next_cursor: null }),
      ),
    );
    server.use(
      http.get('/api/v1/me/snippets', () =>
        HttpResponse.json({ snippets: [], next_cursor: null }),
      ),
    );
    render(<App />);
    // Home is the home page even when signed in (it used to
    // redirect to the dashboard, i.e. "Home goes to Library").
    // Since the Observe theme it is a standalone scene with no
    // shell nav at all - the CTA is the way into the app.
    expect(
      await screen.findByRole('heading', { level: 1, name: /Save a command once/ }, { timeout: 4000 }),
    ).toBeInTheDocument();
    expect(
      await screen.findByText('Open your library', undefined, { timeout: 4000 }),
    ).toBeInTheDocument();

    // Into the shell via the library CTA, then to the profile.
    fireEvent.click(screen.getByText('Open your library'));
    expect(
      await screen.findByRole('button', { name: 'Profile' }, { timeout: 4000 }),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Profile' }));
    expect(
      await screen.findByRole('heading', { name: 'Ada' }, { timeout: 4000 }),
    ).toBeInTheDocument();
    // The profile page lights up Profile, not Home (the old
    // fallback lit Home on every unmatched path).
    expect(screen.getByRole('button', { name: 'Profile' })).toHaveAttribute('aria-current', 'page');
    expect(screen.getByRole('button', { name: 'Home' })).not.toHaveAttribute('aria-current');
  });
});
