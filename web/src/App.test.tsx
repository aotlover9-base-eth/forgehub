import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import App from './App';

describe('App', () => {
  it('renders the shell and the logged-out landing', async () => {
    render(<App />);
    expect(screen.getByRole('navigation', { name: 'Main' })).toBeInTheDocument();
    // /me resolves 401 (MSW default) -> landing with sign-up CTA.
    expect(await screen.findByText('Create account', undefined, { timeout: 4000 })).toBeInTheDocument();
    expect(screen.getByText(/Save a command once/)).toBeInTheDocument();
  });
});

describe('App keyboard shortcuts', () => {
  it('ignores shortcuts while typing in a Material field, fires them outside', async () => {
    render(<App />);
    const cta = await screen.findByText('Create account', undefined, { timeout: 4000 });
    fireEvent.click(cta);
    expect(
      await screen.findByRole('heading', { name: 'Create account' }, { timeout: 4000 }),
    ).toBeInTheDocument();

    // Regression: in a browser, a keydown from a Material
    // field's inner <input> (inside its shadow root) is
    // retargeted to the <md-outlined-text-field> host before
    // it reaches the window listener — dispatch on the host
    // to reproduce that. 'n' is the new-snippet shortcut;
    // an email starting with 'n' bounced the user to /login
    // via /new + RequireAuth on the first letter typed.
    const emailHost = document.querySelector('md-outlined-text-field[label="Email"]');
    expect(emailHost).not.toBeNull();
    fireEvent.keyDown(emailHost as Element, { key: 'n' });
    expect(screen.getByRole('heading', { name: 'Create account' })).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Log in' })).not.toBeInTheDocument();

    // Outside a field the shortcut still works: /new is
    // auth-gated, so signed out we land on the login page.
    fireEvent.keyDown(document.body, { key: 'n' });
    expect(
      await screen.findByRole('heading', { name: 'Log in' }, { timeout: 4000 }),
    ).toBeInTheDocument();
  });
});
