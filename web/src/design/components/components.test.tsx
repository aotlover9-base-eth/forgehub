import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import axe from 'axe-core';
import { Chip, CopyIconButton, StatusChip } from './core';
import { Checkbox, Radio, SelectField, Switch } from './inputs';
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

describe('selection wrappers', () => {
  it('Switch mirrors the selected attribute and reports the new value', () => {
    const onChange = vi.fn();
    const { container } = render(<Switch checked onChange={onChange} label="Soft wrap" />);
    const el = container.querySelector('md-switch') as HTMLElement & { selected: boolean };
    expect(el.hasAttribute('selected')).toBe(true);
    expect(el.getAttribute('aria-label')).toBe('Soft wrap');
    el.selected = false;
    fireEvent.change(el);
    expect(onChange).toHaveBeenCalledWith(false);
  });

  it('Checkbox without a label renders the bare control with its aria label', () => {
    const onChange = vi.fn();
    const { container } = render(<Checkbox checked={false} onChange={onChange} ariaLabel="Select version 2 for diff" />);
    const el = container.querySelector('md-checkbox') as HTMLElement & { checked: boolean };
    expect(el.hasAttribute('checked')).toBe(false);
    expect(el.getAttribute('aria-label')).toBe('Select version 2 for diff');
    el.checked = true;
    fireEvent.change(el);
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it('Radio reports its own value when chosen', () => {
    const onChange = vi.fn();
    const { container } = render(
      <Radio name="theme" value="dark" checked={false} onChange={onChange} label="Dark" />,
    );
    const el = container.querySelector('md-radio') as HTMLElement;
    fireEvent.change(el);
    expect(onChange).toHaveBeenCalledWith('dark');
  });

  it('SelectField renders the options and reports changes', () => {
    const onChange = vi.fn();
    const { container } = render(
      <SelectField
        label="Expires"
        value="never"
        onChange={onChange}
        options={[
          { value: 'never', label: 'Never' },
          { value: '30', label: 'In 30 days' },
        ]}
      />,
    );
    const el = container.querySelector('md-outlined-select') as HTMLElement & { value: string };
    expect(container.querySelectorAll('md-select-option')).toHaveLength(2);
    el.value = '30';
    fireEvent.change(el);
    expect(onChange).toHaveBeenCalledWith('30');
  });
});

describe('CopyIconButton', () => {
  it('morphs to a check after a successful copy', async () => {
    Object.assign(navigator, { clipboard: { writeText: async () => {} } });
    const { container } = render(<CopyIconButton text="fg demo" label="Copy run command" />);
    const button = container.querySelector('md-icon-button') as HTMLElement;
    fireEvent.click(button);
    await waitFor(() => {
      const icon = container.querySelector('.material-symbols-outlined');
      expect(icon?.textContent).toBe('check');
    });
  });
});
