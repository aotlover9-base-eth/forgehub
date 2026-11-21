import { useState, type ReactNode } from 'react';

/** A Material Symbols icon (self-hosted subset font). */
export function Icon({
  name,
  filled = false,
  className,
  slot,
}: {
  name: string;
  filled?: boolean;
  className?: string;
  slot?: string;
}) {
  return (
    <span
      aria-hidden="true"
      slot={slot}
      className={`material-symbols-outlined${className ? ` ${className}` : ''}`}
      style={filled ? { fontVariationSettings: "'FILL' 1, 'wght' 400, 'GRAD' 0, 'opsz' 24" } : undefined}
    >
      {name}
    </span>
  );
}

export type ButtonVariant = 'filled' | 'tonal' | 'outlined' | 'text';

/** Button wrapper over the @material/web button variants. */
export function Button({
  variant = 'filled',
  children,
  onClick,
  disabled,
  type,
  icon,
  href,
}: {
  variant?: ButtonVariant;
  children: ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  type?: 'submit' | 'button';
  icon?: string;
  href?: string;
}) {
  const inner = (
    <>
      {icon ? (
        <span slot="icon">
          <Icon name={icon} />
        </span>
      ) : null}
      {children}
    </>
  );
  const common = { disabled, onClick, href };
  switch (variant) {
    case 'tonal':
      return <md-filled-tonal-button {...common}>{inner}</md-filled-tonal-button>;
    case 'outlined':
      return <md-outlined-button {...common}>{inner}</md-outlined-button>;
    case 'text':
      return <md-text-button {...common}>{inner}</md-text-button>;
    default:
      return (
        <md-filled-button {...common} type={type}>
          {inner}
        </md-filled-button>
      );
  }
}

/** Icon button wrapper (needs an accessible label). */
export function IconButton({
  icon,
  label,
  onClick,
  filled = false,
  disabled,
}: {
  icon: string;
  label: string;
  onClick?: () => void;
  filled?: boolean;
  disabled?: boolean;
}) {
  return (
    <md-icon-button aria-label={label} title={label} onClick={onClick} disabled={disabled}>
      <Icon name={icon} filled={filled} />
    </md-icon-button>
  );
}

/**
 * Copy button with the A6 feedback contract: the icon
 * morphs to a check for a moment on success (the caller
 * usually pairs it with a toast). Clipboard failure leaves
 * the icon unchanged - the text stays selectable.
 */
export function CopyIconButton({
  text,
  label,
  onCopied,
}: {
  text: string;
  label: string;
  onCopied?: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      onCopied?.();
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      // No clipboard permission: nothing to morph.
    }
  };
  return <IconButton icon={copied ? 'check' : 'content_copy'} label={label} onClick={copy} />;
}

/**
 * Floating action button wrapper. With `text`, renders the
 * extended FAB (icon + label) used on expanded layouts; the
 * label doubles as the accessible name.
 */
export function Fab({
  icon,
  label,
  text,
  onClick,
}: {
  icon: string;
  label: string;
  text?: string;
  onClick?: () => void;
}) {
  return (
    <md-fab aria-label={label} label={text} onClick={onClick}>
      <Icon name={icon} slot="icon" />
    </md-fab>
  );
}

/**
 * A neutral tag chip (outlined, surface colours only - the
 * colour-restraint rule: tags are never colourful).
 */
export function Chip({
  label,
  onClick,
  icon,
}: {
  label: string;
  onClick?: () => void;
  icon?: string;
}) {
  return (
    <md-assist-chip label={label} onClick={onClick}>
      {icon ? <Icon name={icon} slot="icon" /> : null}
    </md-assist-chip>
  );
}

export type Visibility = 'public' | 'unlisted' | 'private';

/**
 * The visibility status chip. Per the design spec this is the
 * ONE place tertiary is used (Public); Private uses the
 * secondary-container tonal pair, Unlisted stays neutral.
 */
export function StatusChip({ visibility }: { visibility: Visibility }) {
  const label =
    visibility === 'public' ? 'Public' : visibility === 'unlisted' ? 'Unlisted' : 'Private';
  const icon = visibility === 'public' ? 'public' : visibility === 'unlisted' ? 'link' : 'lock';
  return (
    <span className={`fh-status-chip fh-status-chip--${visibility}`}>
      <Icon name={icon} className="fh-status-chip__icon" />
      {label}
    </span>
  );
}
