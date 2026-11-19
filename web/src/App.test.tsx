import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import App from './App';

describe('App', () => {
  it('renders the shell with navigation and the home placeholder', () => {
    render(<App />);
    expect(screen.getByText('ForgeHub')).toBeInTheDocument();
    expect(screen.getByRole('navigation', { name: 'Main' })).toBeInTheDocument();
    expect(screen.getByText(/arrives in a later step/)).toBeInTheDocument();
  });
});
