import { lazy, Suspense, useEffect, useState, type ReactNode } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  createBrowserRouter,
  Navigate,
  Outlet,
  RouterProvider,
  useLocation,
  useNavigate,
} from 'react-router-dom';
import {
  AppShell,
  Button,
  ConfirmDialog,
  Fab,
  Icon,
  IconButton,
  Menu,
  Skeleton,
  ToastProvider,
  useThemePreference,
  useWidthClass,
  type NavItem,
  type ThemePreference,
} from './design';
import { useLogout, useMe } from './api/hooks';
import { fmt, m } from './i18n/messages';
import { ErrorBoundary } from './ErrorBoundary';
import LandingPage from './pages/LandingPage';
import LoginPage from './pages/LoginPage';
import SignupPage from './pages/SignupPage';
import ForgotPasswordPage from './pages/ForgotPasswordPage';
import ResetPasswordPage from './pages/ResetPasswordPage';
import DashboardPage from './pages/DashboardPage';
import NotFoundPage from './pages/NotFoundPage';

// Route-level code splitting: the heavy screens load on
// demand so the first paint stays small.
const HomePage = lazy(() => import('./pages/HomePage'));
const CollectionPage = lazy(() => import('./pages/CollectionPage'));
const EditorPage = lazy(() => import('./pages/EditorPage'));
const SnippetPage = lazy(() => import('./pages/SnippetPage'));
const ExplorePage = lazy(() => import('./pages/ExplorePage'));
const ProfilePage = lazy(() => import('./pages/ProfilePage'));
const SettingsPage = lazy(() => import('./pages/SettingsPage'));
const DevicePage = lazy(() => import('./pages/DevicePage'));
const DesignPage = lazy(() => import('./pages/DesignPage'));
const RtlTestPage = lazy(() => import('./pages/RtlTestPage'));

const NAV_ITEMS: NavItem[] = [
  { id: 'home', label: m.nav.home, icon: 'home', href: '/home' },
  { id: 'explore', label: m.nav.explore, icon: 'explore', href: '/explore' },
  { id: 'library', label: m.nav.library, icon: 'library_books', href: '/dashboard' },
  { id: 'profile', label: m.nav.profile, icon: 'person', href: '/profile' },
];

/** Gate for signed-in screens: waits for /me, then either
 * renders or redirects to login with a return URL. */
function RequireAuth({ children }: { children: ReactNode }) {
  const me = useMe();
  const location = useLocation();
  if (me.isPending) return null;
  if (!me.data) {
    return <Navigate to={`/login?return=${encodeURIComponent(location.pathname)}`} replace />;
  }
  return <>{children}</>;
}

/** /profile sends you to your own public profile. */
function OwnProfile() {
  const me = useMe();
  if (me.isPending) return null;
  if (!me.data) return <Navigate to="/login?return=/profile" replace />;
  return <Navigate to={`/${me.data.username}`} replace />;
}

/** A slim banner that appears when the browser goes offline. */
function OfflineBanner() {
  const [offline, setOffline] = useState(() => !navigator.onLine);
  useEffect(() => {
    const on = () => setOffline(false);
    const off = () => setOffline(true);
    window.addEventListener('online', on);
    window.addEventListener('offline', off);
    return () => {
      window.removeEventListener('online', on);
      window.removeEventListener('offline', off);
    };
  }, []);
  if (!offline) return null;
  return (
    <div className="fh-offline" role="status">
      <Icon name="info" /> {m.nav.offline}
    </div>
  );
}

const THEME_LABEL: Record<ThemePreference, string> = {
  system: m.nav.themeSystem,
  light: m.nav.themeLight,
  dark: m.nav.themeDark,
};
const NEXT_THEME: Record<ThemePreference, ThemePreference> = {
  system: 'light',
  light: 'dark',
  dark: 'system',
};

/** App bar theme control: cycles System -> Light -> Dark.
 * The full radio group lives in Settings -> Appearance. */
function ThemeButton() {
  const [pref, setPref] = useThemePreference();
  return (
    <Button
      variant="text"
      aria-label={fmt(m.nav.themeAria, { current: THEME_LABEL[pref], next: THEME_LABEL[NEXT_THEME[pref]] })}
      onClick={() => setPref(NEXT_THEME[pref])}
    >
      {fmt(m.nav.themeWithLabel, { label: THEME_LABEL[pref] })}
    </Button>
  );
}

const STATIC_TITLES: Record<string, string> = {
  '/login': m.titles.login,
  '/signup': m.titles.signup,
  '/forgot-password': m.titles.forgot,
  '/reset-password': m.titles.reset,
  '/home': m.titles.home,
  '/dashboard': m.titles.library,
  '/explore': m.titles.explore,
  '/new': m.titles.newSnippet,
  '/settings': m.titles.settings,
  '/device': m.titles.device,
  '/profile': m.titles.profile,
};

function titleForPath(pathname: string): string {
  if (STATIC_TITLES[pathname]) return STATIC_TITLES[pathname];
  const edit = /^\/([^/]+)\/([^/]+)\/edit$/.exec(pathname);
  if (edit) return fmt(m.titles.edit, { name: edit[2] });
  const collection = /^\/collections\/([^/]+)\/([^/]+)$/.exec(pathname);
  if (collection) return fmt(m.titles.collection, { name: collection[2], owner: collection[1] });
  const snippet = /^\/([^/]+)\/([^/]+)$/.exec(pathname);
  if (snippet) return fmt(m.titles.snippet, { name: snippet[2], owner: snippet[1] });
  const profile = /^\/([^/]+)$/.exec(pathname);
  if (profile) return fmt(m.titles.userProfile, { username: profile[1] });
  return m.titles.notFound;
}

/** Per-route document titles (Peak A7). */
function usePageTitle() {
  const { pathname } = useLocation();
  useEffect(() => {
    document.title = titleForPath(pathname);
  }, [pathname]);
}

function Shell() {
  const navigate = useNavigate();
  const location = useLocation();
  const me = useMe();
  const logout = useLogout();
  const [themePref, setThemePref] = useThemePreference();
  const widthClass = useWidthClass();
  const [helpOpen, setHelpOpen] = useState(false);
  usePageTitle();
  // The FAB lives on the two screens where creating is the
  // primary action: your Library and your own profile.
  const showFab =
    Boolean(me.data) &&
    (location.pathname === '/dashboard' || location.pathname === `/${me.data?.username}`);
  // Which nav item is "here": Profile on your own profile
  // page, otherwise the matching section - and nothing on
  // pages that belong to no section (another user's profile,
  // a snippet, settings). Home is the in-app /home page; the
  // public landing at '/' lives outside this shell entirely
  // (the two were separated on 2026-11-20 at the user's
  // direction - Home used to bounce people to the landing).
  const active: string | null =
    me.data && location.pathname === `/${me.data.username}`
      ? 'profile'
      : (NAV_ITEMS.find((item) => location.pathname.startsWith(item.href))?.id ?? null);

  // Minimal keyboard shortcuts (discoverable via the help
  // dialog, opened with ?): n = new snippet, / = focus the
  // dashboard search. Ignored while typing in a field.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // Material fields keep their real <input> inside a
      // shadow root, so a keydown reaching the window is
      // retargeted to the host element and e.target alone
      // looks like "not typing". Walk the composed path
      // (which pierces shadow DOM) and also accept a
      // text-field host itself as typing.
      const typing = e.composedPath().some((node) => {
        if (!(node instanceof HTMLElement)) return false;
        return (
          node instanceof HTMLInputElement ||
          node instanceof HTMLTextAreaElement ||
          node instanceof HTMLSelectElement ||
          node.isContentEditable ||
          node.tagName.endsWith('TEXT-FIELD')
        );
      });
      if (typing || e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key === 'n') {
        e.preventDefault();
        navigate('/new');
      } else if (e.key === '?') {
        e.preventDefault();
        setHelpOpen(true);
      } else if (e.key === '/') {
        const input = document.querySelector<HTMLInputElement>('.fh-searchbar__input');
        if (input) {
          e.preventDefault();
          input.focus();
        }
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [navigate]);

  return (
    <AppShell
      title="ForgeHub"
      navItems={NAV_ITEMS}
      activeId={active}
      onNavigate={(item) => navigate(item.href)}
      actions={
        <>
          <IconButton
            icon="search"
            label={m.nav.searchPublicSnippets}
            onClick={() => navigate('/explore')}
          />
          {me.data ? (
            <Menu
              buttonLabel={fmt(m.nav.accountMenuFor, { username: me.data.username })}
              buttonIcon="account_circle"
              items={[
                { id: 'profile', label: fmt(m.nav.profileWithName, { username: me.data.username }), icon: 'person' },
                { id: 'settings', label: m.nav.settings, icon: 'settings' },
                { id: 'theme', label: fmt(m.nav.themeWithLabel, { label: THEME_LABEL[themePref] }), icon: 'contrast' },
                { id: 'logout', label: m.nav.logOut, icon: 'logout' },
              ]}
              onSelect={(id) => {
                if (!me.data) return;
                if (id === 'profile') navigate(`/${me.data.username}`);
                else if (id === 'settings') navigate('/settings');
                else if (id === 'theme') setThemePref(NEXT_THEME[themePref]);
                else if (id === 'logout')
                  logout.mutate(undefined, { onSuccess: () => navigate('/') });
              }}
            />
          ) : (
            <>
              <ThemeButton />
              <Button variant="text" onClick={() => navigate('/login')}>
                {m.nav.logIn}
              </Button>
            </>
          )}
        </>
      }
      fab={
        showFab ? (
          <span className="fh-fab">
            <Fab
              icon="add"
              label={m.nav.newSnippet}
              text={widthClass === 'expanded' ? m.nav.newSnippet : undefined}
              onClick={() => navigate('/new')}
            />
          </span>
        ) : undefined
      }
      scrollKey={location.pathname}
    >
      <OfflineBanner />
      <ErrorBoundary>
        <Suspense fallback={<Skeleton lines={4} />}>
          {/* Keyed by pathname so the page-transition
              animation replays on every navigation. */}
          <div key={location.pathname} className="fh-route">
            <Outlet />
          </div>
        </Suspense>
      </ErrorBoundary>
      <footer className="fh-footer">
        <span>{m.nav.footerTagline}</span>
        <a href="https://github.com/aotlover9-base-eth/forgehub" target="_blank" rel="noreferrer">
          {m.nav.footerSource}
        </a>
        <span>{m.nav.footerPrivacy}</span>
      </footer>
      <ConfirmDialog
        open={helpOpen}
        headline={m.nav.helpTitle}
        content={
          <ul>
            <li>
              <strong>n</strong> - {m.nav.helpNew}
            </li>
            <li>
              <strong>/</strong> - {m.nav.helpSearch}
            </li>
            <li>
              <strong>?</strong> - {m.nav.helpThis}
            </li>
          </ul>
        }
        confirmLabel={m.nav.helpDone}
        cancelLabel={m.nav.helpClose}
        onConfirm={() => setHelpOpen(false)}
        onCancel={() => setHelpOpen(false)}
      />
    </AppShell>
  );
}

const router = createBrowserRouter([
  // The landing page is a standalone page (Peak A5) with its
  // own top bar - it does not render inside the app shell.
  { path: '/', element: <LandingPage /> },
  {
    element: <Shell />,
    children: [
      { path: 'login', element: <LoginPage /> },
      { path: 'signup', element: <SignupPage /> },
      { path: 'forgot-password', element: <ForgotPasswordPage /> },
      { path: 'reset-password', element: <ResetPasswordPage /> },
      {
        path: 'home',
        element: (
          <RequireAuth>
            <HomePage />
          </RequireAuth>
        ),
      },
      {
        path: 'dashboard',
        element: (
          <RequireAuth>
            <DashboardPage />
          </RequireAuth>
        ),
      },
      {
        path: 'new',
        element: (
          <RequireAuth>
            <EditorPage />
          </RequireAuth>
        ),
      },
      { path: 'explore', element: <ExplorePage /> },
      { path: 'collections/:owner/:name', element: <CollectionPage /> },
      { path: 'device', element: <DevicePage /> },
      {
        path: 'settings',
        element: (
          <RequireAuth>
            <SettingsPage />
          </RequireAuth>
        ),
      },
      { path: 'profile', element: <OwnProfile /> },
      { path: ':username', element: <ProfilePage /> },
      { path: ':owner/:name', element: <SnippetPage /> },
      {
        path: ':owner/:name/edit',
        element: (
          <RequireAuth>
            <EditorPage />
          </RequireAuth>
        ),
      },
      ...(import.meta.env.DEV
        ? [
            { path: '__design', element: <DesignPage /> },
            { path: '__rtl', element: <RtlTestPage /> },
          ]
        : []),
      { path: '*', element: <NotFoundPage /> },
    ],
  },
]);

const queryClient = new QueryClient({
  defaultOptions: { queries: { staleTime: 30_000 } },
});

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <RouterProvider router={router} />
      </ToastProvider>
    </QueryClientProvider>
  );
}
