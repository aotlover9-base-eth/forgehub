import { render, screen } from '@testing-library/react';
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
