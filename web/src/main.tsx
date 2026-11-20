import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import './design/md';
import './index.css';
import App from './App.tsx';

// ForgeHub ships exactly one theme - the dark Observe theme -
// pinned as data-theme="dark" on <html> in index.html. There
// is no runtime theme switching (removed 2026-11-20: inside
// the Observe app the old light/dark toggle changed nothing,
// and a control that lies is worse than no control).

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
