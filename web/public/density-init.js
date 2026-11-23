// Density bootstrap, loaded before the app bundle so the
// first paint already uses the stored list density: the
// preference is 'comfortable' | 'compact' in localStorage,
// mirrored onto <html data-density>. src/design/density.ts
// owns it at runtime. This must be an external file: the
// server CSP is script-src 'self', which forbids inline
// scripts. (The theme needs no bootstrap: the product ships
// exactly one theme, the dark Observe theme, pinned as
// data-theme="dark" on <html> in index.html.)
(function () {
  var density = 'comfortable';
  try {
    if (window.localStorage.getItem('forgehub-density') === 'compact') {
      density = 'compact';
    }
  } catch (err) {
    // Storage unavailable: comfortable default.
  }
  document.documentElement.setAttribute('data-density', density);
})();
