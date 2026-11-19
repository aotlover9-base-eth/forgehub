import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import './design/md';
import { applyTheme, readThemePreference } from './design/theme';
import './index.css';
import App from './App.tsx';

// The pre-paint script in index.html already applied the
// stored theme; re-applying here keeps it correct if the
// document was served from a cache without the script.
applyTheme(readThemePreference());

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
