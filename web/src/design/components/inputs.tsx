import { m } from '../../i18n/messages';
import { useId, useRef, useState, type ReactNode } from 'react';
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
  readOnly,
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
  readOnly?: boolean;
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
      readonly={readOnly}
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
  placeholder = m.design.search,
  ariaLabel = m.design.search,
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
        <IconButton icon="close" label={m.design.clearSearch} onClick={() => onChange('')} />
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

/**
 * Selection controls. Pages used to drop raw md-switch /
 * md-checkbox / md-radio elements inline; these wrappers own
 * the labelled-row anatomy (48 px row, token gap, pointer
 * cursor) so every control reads the same and carries its
 * label as a real <label>.
 */
export function Switch({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: ReactNode;
  disabled?: boolean;
}) {
  return (
    <label className="fh-select-row">
      <md-switch
        selected={checked || undefined}
        disabled={disabled}
        aria-label={typeof label === 'string' ? label : undefined}
        onChange={(e) => onChange((e.target as HTMLElement & { selected: boolean }).selected)}
      />
      <span>{label}</span>
    </label>
  );
}

export function Checkbox({
  checked,
  onChange,
  label,
  ariaLabel,
  disabled,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label?: ReactNode;
  ariaLabel?: string;
  disabled?: boolean;
}) {
  const box = (
    <md-checkbox
      checked={checked || undefined}
      disabled={disabled}
      aria-label={ariaLabel ?? (typeof label === 'string' ? label : undefined)}
      onChange={(e) => onChange((e.target as HTMLElement & { checked: boolean }).checked)}
    />
  );
  if (label === undefined) return box;
  return (
    <label className="fh-select-row">
      {box}
      <span>{label}</span>
    </label>
  );
}

export function Radio({
  name,
  value,
  checked,
  onChange,
  label,
  disabled,
}: {
  name: string;
  value: string;
  checked: boolean;
  onChange: (value: string) => void;
  label: ReactNode;
  disabled?: boolean;
}) {
  return (
    <label className="fh-select-row fh-select-row--start">
      <md-radio
        name={name}
        value={value}
        checked={checked || undefined}
        disabled={disabled}
        aria-label={typeof label === 'string' ? label : undefined}
        onChange={() => onChange(value)}
      />
      <span>{label}</span>
    </label>
  );
}

/**
 * Outlined select over md-outlined-select (the M3 menu
 * button field), replacing the one raw <select> the app had.
 */
export function SelectField({
  label,
  value,
  onChange,
  options,
  ariaLabel,
}: {
  label?: string;
  value: string;
  onChange: (value: string) => void;
  options: { value: string; label: string }[];
  ariaLabel?: string;
}) {
  return (
    <md-outlined-select
      label={label}
      aria-label={ariaLabel}
      value={value}
      onChange={(e) => onChange((e.target as HTMLElement & { value: string }).value)}
    >
      {options.map((option) => (
        <md-select-option key={option.value} value={option.value}>
          <span slot="headline">{option.label}</span>
        </md-select-option>
      ))}
    </md-outlined-select>
  );
}

/**
 * Segmented control (M3 segmented buttons, single select):
 * a short closed choice - visibility in the editor, sort on
 * Explore. Selection is controlled; each button reports its
 * own id.
 */
export function SegmentedControl<T extends string>({
  options,
  value,
  onChange,
  ariaLabel,
}: {
  options: { id: T; label: string; icon?: string }[];
  value: T;
  onChange: (value: T) => void;
  ariaLabel: string;
}) {
  return (
    <md-outlined-segmented-button-set aria-label={ariaLabel}>
      {options.map((option) => (
        <md-outlined-segmented-button
          key={option.id}
          label={option.label}
          selected={value === option.id || undefined}
          onClick={() => onChange(option.id)}
        >
          {option.icon ? <Icon name={option.icon} slot="icon" /> : null}
        </md-outlined-segmented-button>
      ))}
    </md-outlined-segmented-button-set>
  );
}
