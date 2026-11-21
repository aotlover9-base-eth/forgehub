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
const EditorPage = lazy(() => import('./pages/EditorPage'));
const SnippetPage = lazy(() => import('./pages/SnippetPage'));
const ExplorePage = lazy(() => import('./pages/ExplorePage'));
const ProfilePage = lazy(() => import('./pages/ProfilePage'));
const SettingsPage = lazy(() => import('./pages/SettingsPage'));
const DevicePage = lazy(() => import('./pages/DevicePage'));
const DesignPage = lazy(() => import('./pages/DesignPage'));

const NAV_ITEMS: NavItem[] = [
  { id: 'home', label: 'Home', icon: 'home', href: '/home' },
  { id: 'explore', label: 'Explore', icon: 'explore', href: '/explore' },
  { id: 'library', label: 'Library', icon: 'library_books', href: '/dashboard' },
  { id: 'profile', label: 'Profile', icon: 'person', href: '/profile' },
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
      <Icon name="info" /> You are offline. Changes will fail until the connection is back.
    </div>
  );
}

const THEME_LABEL: Record<ThemePreference, string> = {
  system: 'System',
  light: 'Light',
  dark: 'Dark',
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
      aria-label={`Theme: ${THEME_LABEL[pref]}. Activate to switch to ${THEME_LABEL[NEXT_THEME[pref]]}.`}
      onClick={() => setPref(NEXT_THEME[pref])}
    >
      Theme: {THEME_LABEL[pref]}
    </Button>
  );
}

const STATIC_TITLES: Record<string, string> = {
  '/login': 'Log in - ForgeHub',
  '/signup': 'Create account - ForgeHub',
  '/forgot-password': 'Reset your password - ForgeHub',
  '/reset-password': 'Set a new password - ForgeHub',
  '/home': 'Home - ForgeHub',
  '/dashboard': 'Library - ForgeHub',
  '/explore': 'Explore - ForgeHub',
  '/new': 'New snippet - ForgeHub',
  '/settings': 'Settings - ForgeHub',
  '/device': 'Approve a device - ForgeHub',
  '/profile': 'Your profile - ForgeHub',
};

function titleForPath(pathname: string): string {
  if (STATIC_TITLES[pathname]) return STATIC_TITLES[pathname];
  const edit = /^\/([^/]+)\/([^/]+)\/edit$/.exec(pathname);
  if (edit) return `Edit ${edit[2]} - ForgeHub`;
  const snippet = /^\/([^/]+)\/([^/]+)$/.exec(pathname);
  if (snippet) return `${snippet[2]} by ${snippet[1]} - ForgeHub`;
  const profile = /^\/([^/]+)$/.exec(pathname);
  if (profile) return `${profile[1]} - ForgeHub`;
  return 'Not found - ForgeHub';
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
            label="Search public snippets"
            onClick={() => navigate('/explore')}
          />
          {me.data ? (
            <Menu
              buttonLabel={`Account menu for ${me.data.username}`}
              buttonIcon="account_circle"
              items={[
                { id: 'profile', label: `Profile (${me.data.username})`, icon: 'person' },
                { id: 'settings', label: 'Settings', icon: 'settings' },
                { id: 'theme', label: `Theme: ${THEME_LABEL[themePref]}`, icon: 'contrast' },
                { id: 'logout', label: 'Log out', icon: 'logout' },
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
                Log in
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
              label="New snippet"
              text={widthClass === 'expanded' ? 'New snippet' : undefined}
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
        <span>ForgeHub - save a command once, run it anywhere.</span>
        <a href="https://github.com/aotlover9-base-eth/forgehub" target="_blank" rel="noreferrer">
          Source
        </a>
        <span>No tracking. No third-party scripts.</span>
      </footer>
      <ConfirmDialog
        open={helpOpen}
        headline="Keyboard shortcuts"
        content={
          <ul>
            <li>
              <strong>n</strong> - new snippet
            </li>
            <li>
              <strong>/</strong> - focus search (on the dashboard)
            </li>
            <li>
              <strong>?</strong> - this help
            </li>
          </ul>
        }
        confirmLabel="Done"
        cancelLabel="Close"
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
      ...(import.meta.env.DEV ? [{ path: '__design', element: <DesignPage /> }] : []),
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
