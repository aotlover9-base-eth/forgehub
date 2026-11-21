import { useEffect, useRef, type ReactNode } from 'react';
import { useWidthClass } from '../hooks';
import { Icon } from './core';

export type NavItem = {
  id: string;
  label: string;
  icon: string;
  href: string;
};

/**
 * The app shell: top app bar everywhere; navigation as a
 * bottom bar (compact), a rail (medium), or an extended rail
 * (expanded) - the Material 3 adaptive pattern. Exactly one
 * navigation element is rendered per width class.
 */
export function AppShell({
  title,
  navItems,
  activeId,
  onNavigate,
  actions,
  fab,
  scrollKey,
  children,
}: {
  title: string;
  navItems: NavItem[];
  activeId: string | null;
  onNavigate: (item: NavItem) => void;
  actions?: ReactNode;
  /** Floating action button, rendered above the content at
   * the trailing-bottom corner (clears the navigation bar on
   * compact). */
  fab?: ReactNode;
  /** When this changes, the content column scrolls back to
   * the top (route changes pass the pathname). */
  scrollKey?: string;
  children: ReactNode;
}) {
  const widthClass = useWidthClass();
  const mainRef = useRef<HTMLElement>(null);
  useEffect(() => {
    // Optional call: some DOM implementations (jsdom in
    // tests) have no Element.scrollTo.
    mainRef.current?.scrollTo?.({ top: 0 });
  }, [scrollKey]);
  const navButton = (item: NavItem, showLabelBeside = false) => {
    const active = item.id === activeId;
    return (
      <button
        key={item.id}
        className={`fh-nav__item${active ? ' fh-nav__item--active' : ''}${showLabelBeside ? ' fh-nav__item--beside' : ''}`}
        aria-current={active ? 'page' : undefined}
        onClick={() => onNavigate(item)}
      >
        <span className="fh-nav__pill">
          <Icon name={item.icon} filled={active} />
        </span>
        <span className="fh-nav__label">{item.label}</span>
      </button>
    );
  };
  return (
    <div className={`fh-shell fh-shell--${widthClass}`}>
      <header className={`fh-topbar${widthClass === 'expanded' ? ' fh-topbar--medium' : ''}`}>
        <span className="fh-topbar__title">{title}</span>
        <span className="fh-topbar__actions">{actions}</span>
      </header>
      {widthClass === 'expanded' ? (
        <nav className="fh-nav fh-nav--extended" aria-label="Main">
          {navItems.map((item) => navButton(item, true))}
        </nav>
      ) : widthClass === 'medium' ? (
        <nav className="fh-nav fh-nav--rail" aria-label="Main">
          {navItems.map((item) => navButton(item))}
        </nav>
      ) : null}
      <main className="fh-shell__main" ref={mainRef}>
        {children}
      </main>
      {fab}
      {widthClass === 'compact' ? (
        <nav className="fh-nav fh-nav--bar" aria-label="Main">
          {navItems.map((item) => navButton(item))}
        </nav>
      ) : null}
    </div>
  );
}
