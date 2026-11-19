import { useId, useRef, useState } from 'react';
import { Icon, IconButton } from './core';

/**
 * Text field wrapper over md-outlined-text-field with proper
 * error text (the field's `error` + `error-text`).
 */
export function TextField({
  label,
  value,
  onChange,
  errorText,
  supportingText,
  type = 'text',
  required,
  disabled,
  autoComplete,
  multiline,
  rows,
  maxLength,
  name,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  errorText?: string;
  supportingText?: string;
  type?: string;
  required?: boolean;
  disabled?: boolean;
  autoComplete?: string;
  multiline?: boolean;
  rows?: number;
  maxLength?: number;
  name?: string;
}) {
  return (
    <md-outlined-text-field
      label={label}
      value={value}
      type={multiline ? 'textarea' : type}
      required={required}
      disabled={disabled}
      autoComplete={autoComplete}
      error={errorText ? true : undefined}
      error-text={errorText}
      supporting-text={errorText ? undefined : supportingText}
      rows={multiline ? (rows ?? 4) : undefined}
      maxLength={maxLength}
      name={name}
      onInput={(e) => onChange((e.target as HTMLInputElement).value)}
      style={{ width: '100%' }}
    />
  );
}

/**
 * The M3 search bar: a surface-container pill with a leading
 * search icon, the input, and a clear button when non-empty.
 * Submits on Enter.
 */
export function SearchBar({
  value,
  onChange,
  onSubmit,
  placeholder = 'Search',
  ariaLabel = 'Search',
}: {
  value: string;
  onChange: (value: string) => void;
  onSubmit?: (value: string) => void;
  placeholder?: string;
  ariaLabel?: string;
}) {
  return (
    <form
      className="fh-searchbar"
      role="search"
      onSubmit={(e) => {
        e.preventDefault();
        onSubmit?.(value);
      }}
    >
      <Icon name="search" className="fh-searchbar__icon" />
      <input
        className="fh-searchbar__input"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        aria-label={ariaLabel}
        type="search"
      />
      {value ? (
        <IconButton icon="close" label="Clear search" onClick={() => onChange('')} />
      ) : null}
    </form>
  );
}

/** Tabs wrapper over md-tabs (primary indicator style). */
export function Tabs({
  tabs,
  activeIndex,
  onChange,
  ariaLabel,
}: {
  tabs: { id: string; label: string; icon?: string }[];
  activeIndex: number;
  onChange: (index: number) => void;
  ariaLabel?: string;
}) {
  return (
    <md-tabs
      aria-label={ariaLabel}
      active-tab-index={activeIndex}
      onChange={(e) => {
        const target = e.target as HTMLElement & { activeTabIndex: number };
        onChange(target.activeTabIndex);
      }}
    >
      {tabs.map((tab) => (
        <md-primary-tab key={tab.id} aria-label={tab.label}>
          {tab.icon ? <Icon name={tab.icon} slot="icon" /> : null}
          {tab.label}
        </md-primary-tab>
      ))}
    </md-tabs>
  );
}

/**
 * A simple anchored menu: an icon button toggling an md-menu.
 * Items call onSelect with their id.
 */
export function Menu({
  buttonLabel,
  buttonIcon = 'more_vert',
  items,
  onSelect,
}: {
  buttonLabel: string;
  buttonIcon?: string;
  items: { id: string; label: string; icon?: string }[];
  onSelect: (id: string) => void;
}) {
  const anchorId = useId();
  const menuRef = useRef<HTMLElement & { open: boolean }>(null);
  const [open, setOpen] = useState(false);
  const toggle = () => {
    const next = !open;
    setOpen(next);
    if (menuRef.current) menuRef.current.open = next;
  };
  return (
    <span style={{ position: 'relative', display: 'inline-block' }}>
      <md-icon-button id={anchorId} aria-label={buttonLabel} title={buttonLabel} onClick={toggle}>
        <Icon name={buttonIcon} />
      </md-icon-button>
      <md-menu
        ref={menuRef as never}
        anchor={anchorId}
        positioning="popover"
        onClose={() => setOpen(false)}
      >
        {items.map((item) => (
          <md-menu-item
            key={item.id}
            onClick={() => {
              setOpen(false);
              onSelect(item.id);
            }}
          >
            <span slot="headline">{item.label}</span>
            {item.icon ? <Icon name={item.icon} slot="start" /> : null}
          </md-menu-item>
        ))}
      </md-menu>
    </span>
  );
}
