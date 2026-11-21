import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import './design/md';
import './index.css';
import App from './App.tsx';

// The theme preference (system / light / dark) is resolved
// before first paint by public/theme-init.js and owned at
// runtime by src/design/theme.ts (Peak A1, 2026-11-22). The
// single-theme Observe pin that used to live here is gone.

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
