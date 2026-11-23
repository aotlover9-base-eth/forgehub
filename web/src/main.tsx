import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import './design/md';
import './index.css';
import App from './App.tsx';

// ForgeHub ships exactly one theme - the dark Observe theme -
// pinned as data-theme="dark" on <html> in index.html. There
// is no runtime theme switching (restored 2026-11-23 at the
// owner's direction: inside the Observe app a light/dark
// toggle changes nothing that matters, and a control that
// lies is worse than no control). The list density is the
// only display preference; public/density-init.js applies it
// before first paint and src/design/density.ts owns it after.

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
