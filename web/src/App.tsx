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
  Icon,
  Skeleton,
  ToastProvider,
  type NavItem,
} from './design';
import { useLogout, useMe } from './api/hooks';
import { ErrorBoundary } from './ErrorBoundary';
import LandingPage from './pages/LandingPage';
import LoginPage from './pages/LoginPage';
import SignupPage from './pages/SignupPage';
import DashboardPage from './pages/DashboardPage';
import NotFoundPage from './pages/NotFoundPage';

// Route-level code splitting: the heavy screens load on
// demand so the first paint stays small.
const EditorPage = lazy(() => import('./pages/EditorPage'));
const SnippetPage = lazy(() => import('./pages/SnippetPage'));
const ExplorePage = lazy(() => import('./pages/ExplorePage'));
const ProfilePage = lazy(() => import('./pages/ProfilePage'));
const SettingsPage = lazy(() => import('./pages/SettingsPage'));
const DevicePage = lazy(() => import('./pages/DevicePage'));
const DesignPage = lazy(() => import('./pages/DesignPage'));

const NAV_ITEMS: NavItem[] = [
  { id: 'home', label: 'Home', icon: 'home', href: '/' },
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

function Shell() {
  const navigate = useNavigate();
  const location = useLocation();
  const me = useMe();
  const logout = useLogout();
  const [helpOpen, setHelpOpen] = useState(false);
  // Which nav item is "here": Home only at '/', Profile on
  // your own profile page, otherwise the matching section —
  // and nothing on pages that belong to no section (another
  // user's profile, a snippet, settings). The old fallback
  // lit up Home on all of those, which made Profile look
  // like it had gone Home.
  const active: string | null =
    location.pathname === '/'
      ? 'home'
      : me.data && location.pathname === `/${me.data.username}`
        ? 'profile'
        : (NAV_ITEMS.find(
            (item) => item.href !== '/' && location.pathname.startsWith(item.href),
          )?.id ?? null);

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
        me.data ? (
          <>
            <span aria-label={`Signed in as ${me.data.username}`}>{me.data.username}</span>
            <Button variant="text" onClick={() => navigate('/settings')}>
              Settings
            </Button>
            <Button
              variant="text"
              onClick={() => logout.mutate(undefined, { onSuccess: () => navigate('/') })}
            >
              Log out
            </Button>
          </>
        ) : (
          <Button variant="text" onClick={() => navigate('/login')}>
            Log in
          </Button>
        )
      }
    >
      <OfflineBanner />
      <ErrorBoundary>
        <Suspense fallback={<Skeleton lines={4} />}>
          <Outlet />
        </Suspense>
      </ErrorBoundary>
      <footer className="fh-footer">
        <span>ForgeHub — save a command once, run it anywhere.</span>
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
              <strong>n</strong> — new snippet
            </li>
            <li>
              <strong>/</strong> — focus search (on the dashboard)
            </li>
            <li>
              <strong>?</strong> — this help
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
  {
    path: '/',
    element: <Shell />,
    children: [
      { index: true, element: <LandingPage /> },
      { path: 'login', element: <LoginPage /> },
      { path: 'signup', element: <SignupPage /> },
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
