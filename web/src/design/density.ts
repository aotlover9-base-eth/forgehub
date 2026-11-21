import { useSyncExternalStore } from 'react';

/**
 * List density preference (Peak A5): comfortable (default)
 * or compact rows in snippet lists. Stored per device in
 * localStorage, mirrored onto <html data-density> so
 * components.css can tighten row spacing. public/theme-init.js
 * applies the stored value before first paint; this module
 * owns it at runtime (same pattern as theme.ts).
 */
export type Density = 'comfortable' | 'compact';

const STORAGE_SLOT = 'forgehub-density';
const listeners = new Set<() => void>();

function readStored(): Density {
  try {
    return window.localStorage.getItem(STORAGE_SLOT) === 'compact' ? 'compact' : 'comfortable';
  } catch {
    return 'comfortable';
  }
}

let current: Density = readStored();

function apply(density: Density) {
  current = density;
  document.documentElement.dataset.density = density;
  try {
    window.localStorage.setItem(STORAGE_SLOT, density);
  } catch {
    // Private mode: the choice lasts for the session.
  }
  listeners.forEach((listener) => listener());
}

export function setDensity(density: Density) {
  apply(density);
}

export function useDensity(): [Density, (density: Density) => void] {
  const density = useSyncExternalStore(
    (onStoreChange) => {
      listeners.add(onStoreChange);
      return () => {
        listeners.delete(onStoreChange);
      };
    },
    () => current,
    () => current,
  );
  return [density, setDensity];
}
