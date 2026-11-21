import { useState, type ReactNode } from 'react';
import { Button, Icon, IconButton } from './core';

/** Page header: headline + optional subtitle + actions. */
export function PageHeader({
  title,
  subtitle,
  actions,
}: {
  title: string;
  subtitle?: string;
  actions?: ReactNode;
}) {
  return (
    <div className="fh-page-header">
      <div className="fh-page-header__text">
        <h1 className="fh-page-header__title">{title}</h1>
        {subtitle ? <p className="fh-page-header__subtitle">{subtitle}</p> : null}
      </div>
      {actions ? <div className="fh-page-header__actions">{actions}</div> : null}
    </div>
  );
}

/** Card in the three M3 variants. */
export function Card({
  variant = 'filled',
  children,
  className,
}: {
  variant?: 'filled' | 'outlined' | 'elevated';
  children: ReactNode;
  className?: string;
}) {
  return <div className={`fh-card fh-card--${variant}${className ? ` ${className}` : ''}`}>{children}</div>;
}

/** A list row with headline, supporting text and slots. */
export function ListItem({
  headline,
  supportingText,
  leading,
  trailing,
  onClick,
}: {
  headline: string;
  supportingText?: string;
  leading?: ReactNode;
  trailing?: ReactNode;
  onClick?: () => void;
}) {
  return (
    <md-list-item onClick={onClick} type={onClick ? 'button' : 'text'}>
      <span slot="headline">{headline}</span>
      {supportingText ? <span slot="supporting-text">{supportingText}</span> : null}
      {leading ? <span slot="start">{leading}</span> : null}
      {trailing ? <span slot="end">{trailing}</span> : null}
    </md-list-item>
  );
}

/**
 * Code block: Roboto Mono on the code token background, long
 * lines wrapped (never truncated or hidden), optional line
 * numbers, and a copy button with a copied confirmation.
 */
export function CodeBlock({
  code,
  lineNumbers = false,
  label = 'Code',
}: {
  code: string;
  lineNumbers?: boolean;
  label?: string;
}) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard unavailable (permissions): the text stays
      // selectable, which is the fallback.
    }
  };
  const lines = code.replace(/\n$/, '').split('\n');
  return (
    <div className="fh-codeblock">
      <div className="fh-codeblock__bar">
        <span className="fh-codeblock__label">{label}</span>
        <IconButton icon={copied ? 'check' : 'content_copy'} label="Copy code" onClick={copy} />
      </div>
      <pre className="fh-codeblock__pre">
        {lines.map((line, i) => (
          <span className="fh-codeblock__line" key={i}>
            {lineNumbers ? <span className="fh-codeblock__num">{i + 1}</span> : null}
            <code>{line === '' ? ' ' : line}</code>
          </span>
        ))}
      </pre>
    </div>
  );
}

/** Empty state: icon, title, guidance, optional action. */
export function EmptyState({
  icon,
  title,
  message,
  actionLabel,
  onAction,
  titleLevel = 2,
}: {
  icon: string;
  title: string;
  message: string;
  actionLabel?: string;
  onAction?: () => void;
  /** Standalone pages (404, missing profile) pass 1 so the
   * page keeps a proper heading outline. */
  titleLevel?: 1 | 2;
}) {
  const Title = titleLevel === 1 ? 'h1' : 'h2';
  return (
    <div className="fh-state">
      <Icon name={icon} className="fh-state__icon" />
      <Title className="fh-state__title">{title}</Title>
      <p className="fh-state__message">{message}</p>
      {actionLabel && onAction ? (
        <Button variant="tonal" onClick={onAction}>
          {actionLabel}
        </Button>
      ) : null}
    </div>
  );
}

/** Error state: the error colour is used here and only for
 * errors (colour-restraint rule). */
export function ErrorState({
  title,
  message,
  retryLabel,
  onRetry,
}: {
  title: string;
  message: string;
  retryLabel?: string;
  onRetry?: () => void;
}) {
  return (
    <div className="fh-state" role="alert">
      <Icon name="error" className="fh-state__icon fh-state__icon--error" />
      <h2 className="fh-state__title">{title}</h2>
      <p className="fh-state__message">{message}</p>
      {retryLabel && onRetry ? (
        <Button variant="outlined" onClick={onRetry}>
          {retryLabel}
        </Button>
      ) : null}
    </div>
  );
}

/** A pulsing placeholder block while content loads. */
export function Skeleton({ lines = 3 }: { lines?: number }) {
  return (
    <div className="fh-skeleton" aria-hidden="true">
      {Array.from({ length: lines }, (_, i) => (
        <div className="fh-skeleton__line" key={i} style={{ width: `${92 - i * 13}%` }} />
      ))}
    </div>
  );
}
