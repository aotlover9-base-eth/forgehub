// Theme bootstrap, loaded before the app bundle so the first
// paint already uses the right scheme (no flash of the wrong
// theme). It mirrors src/design/theme.ts: the preference is
// 'system' | 'light' | 'dark' in localStorage; system resolves
// through prefers-color-scheme. This must be an external file:
// the server CSP is script-src 'self', which forbids inline
// scripts.
(function () {
  var pref = 'system';
  try {
    var stored = window.localStorage.getItem('forgehub-theme');
    if (stored === 'light' || stored === 'dark' || stored === 'system') {
      pref = stored;
    }
  } catch (err) {
    // Storage unavailable (private mode, disabled cookies):
    // fall through with the system preference.
  }
  var dark =
    pref === 'dark' ||
    (pref === 'system' &&
      typeof window.matchMedia === 'function' &&
      window.matchMedia('(prefers-color-scheme: dark)').matches);
  var resolved = dark ? 'dark' : 'light';
  document.documentElement.setAttribute('data-theme', resolved);
  document.documentElement.style.colorScheme = resolved;
})();
