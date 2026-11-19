import { useEffect, useState, type ReactNode } from 'react';
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
  EmptyState,
  ToastProvider,
  type NavItem,
} from './design';
import { useLogout, useMe } from './api/hooks';
import LandingPage from './pages/LandingPage';
import LoginPage from './pages/LoginPage';
import SignupPage from './pages/SignupPage';
import DashboardPage from './pages/DashboardPage';
import EditorPage from './pages/EditorPage';
import SnippetPage from './pages/SnippetPage';
import DesignPage from './pages/DesignPage';

const NAV_ITEMS: NavItem[] = [
  { id: 'home', label: 'Home', icon: 'home', href: '/' },
  { id: 'explore', label: 'Explore', icon: 'explore', href: '/explore' },
  { id: 'library', label: 'Library', icon: 'library_books', href: '/dashboard' },
  { id: 'profile', label: 'Profile', icon: 'person', href: '/profile' },
];

function Placeholder({ title }: { title: string }) {
  return <EmptyState icon="terminal" title={title} message="This screen arrives in a later step." />;
}

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

function Shell() {
  const navigate = useNavigate();
  const location = useLocation();
  const me = useMe();
  const logout = useLogout();
  const [helpOpen, setHelpOpen] = useState(false);
  const active =
    NAV_ITEMS.find((item) => item.href !== '/' && location.pathname.startsWith(item.href))?.id ??
    'home';

  // Minimal keyboard shortcuts (discoverable via the help
  // dialog, opened with ?): n = new snippet, / = focus the
  // dashboard search. Ignored while typing in a field.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const typing =
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target?.isContentEditable;
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
      <Outlet />
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
      { path: 'explore', element: <Placeholder title="Explore" /> },
      { path: 'profile', element: <Placeholder title="Profile" /> },
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
      { path: '*', element: <Placeholder title="Not found" /> },
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
