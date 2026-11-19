import { type ReactNode } from 'react';
import { useWidthClass } from '../hooks';
import { readThemePreference, setThemePreference, type ThemePreference } from '../theme';
import { Icon, IconButton } from './core';
import { useState } from 'react';

export type NavItem = {
  id: string;
  label: string;
  icon: string;
  href: string;
};

/**
 * The app shell: top app bar everywhere; navigation as a
 * bottom bar (compact), a rail (medium), or an extended rail
 * (expanded) — the Material 3 adaptive pattern. Exactly one
 * navigation element is rendered per width class.
 */
export function AppShell({
  title,
  navItems,
  activeId,
  onNavigate,
  actions,
  children,
}: {
  title: string;
  navItems: NavItem[];
  activeId: string;
  onNavigate: (item: NavItem) => void;
  actions?: ReactNode;
  children: ReactNode;
}) {
  const widthClass = useWidthClass();
  const [theme, setTheme] = useState<ThemePreference>(() => readThemePreference());
  const cycleTheme = () => {
    const order: ThemePreference[] = ['system', 'light', 'dark'];
    const next = order[(order.indexOf(theme) + 1) % order.length];
    setTheme(next);
    setThemePreference(next);
  };
  const themeIcon = theme === 'light' ? 'light_mode' : theme === 'dark' ? 'dark_mode' : 'contrast';
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
      <header className="fh-topbar">
        <span className="fh-topbar__title">{title}</span>
        <span className="fh-topbar__actions">
          {actions}
          <IconButton
            icon={themeIcon}
            label={`Theme: ${theme}. Activate to change.`}
            onClick={cycleTheme}
          />
        </span>
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
      <main className="fh-shell__main">{children}</main>
      {widthClass === 'compact' ? (
        <nav className="fh-nav fh-nav--bar" aria-label="Main">
          {navItems.map((item) => navButton(item))}
        </nav>
      ) : null}
    </div>
  );
}
