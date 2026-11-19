import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import axe from 'axe-core';
import { Chip, StatusChip } from './core';
import { CodeBlock, EmptyState, ErrorState } from './surfaces';

describe('StatusChip', () => {
  it('uses the tertiary treatment ONLY for Public', () => {
    const { rerender } = render(<StatusChip visibility="public" />);
    expect(screen.getByText('Public').className).toContain('fh-status-chip--public');
    rerender(<StatusChip visibility="private" />);
    expect(screen.getByText('Private').className).toContain('fh-status-chip--private');
    expect(screen.getByText('Private').className).not.toContain('--public');
    rerender(<StatusChip visibility="unlisted" />);
    expect(screen.getByText('Unlisted').className).toContain('fh-status-chip--unlisted');
  });
});

describe('Chip', () => {
  it('renders an assist chip carrying the label', () => {
    // md-assist-chip renders its label into its shadow root in
    // a real browser; the wrapper contract is the element +
    // label attribute, which is what we assert in jsdom.
    const { container } = render(<Chip label="docker" />);
    const chip = container.querySelector('md-assist-chip');
    expect(chip?.getAttribute('label')).toBe('docker');
  });
});

describe('CodeBlock', () => {
  it('renders every line and a copy control', () => {
    const { container } = render(<CodeBlock code={'echo one\necho two'} lineNumbers label="demo · v1" />);
    expect(screen.getByText('echo one')).toBeTruthy();
    expect(screen.getByText('echo two')).toBeTruthy();
    expect(container.querySelector('md-icon-button[aria-label="Copy code"]')).toBeTruthy();
    expect(screen.getByText('demo · v1')).toBeTruthy();
  });
});

describe('states', () => {
  it('EmptyState explains and offers its action', () => {
    render(
      <EmptyState icon="search" title="Nothing found" message="Try a different term." actionLabel="Clear" onAction={() => {}} />,
    );
    expect(screen.getByText('Nothing found')).toBeTruthy();
    expect(screen.getByText('Try a different term.')).toBeTruthy();
  });

  it('ErrorState is an alert with a retry', () => {
    render(<ErrorState title="Failed" message="Server error" retryLabel="Retry" onRetry={() => {}} />);
    expect(screen.getByRole('alert')).toBeTruthy();
    expect(screen.getByText('Retry')).toBeTruthy();
  });
});

describe('accessibility (axe)', () => {
  it('has no violations on the state components', async () => {
    const { container } = render(
      <div>
        <StatusChip visibility="public" />
        <EmptyState icon="search" title="Nothing" message="Empty here." />
        <CodeBlock code="echo hi" />
      </div>,
    );
    const results = await axe.run(container);
    expect(results.violations).toEqual([]);
  });
});
