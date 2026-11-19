import { BrowserRouter, Route, Routes, useLocation, useNavigate } from 'react-router-dom';
import { AppShell, EmptyState, ToastProvider, type NavItem } from './design';
import DesignPage from './pages/DesignPage';

const NAV_ITEMS: NavItem[] = [
  { id: 'home', label: 'Home', icon: 'home', href: '/' },
  { id: 'explore', label: 'Explore', icon: 'explore', href: '/explore' },
  { id: 'library', label: 'Library', icon: 'library_books', href: '/library' },
  { id: 'profile', label: 'Profile', icon: 'person', href: '/profile' },
];

function Placeholder({ title }: { title: string }) {
  return (
    <EmptyState
      icon="terminal"
      title={title}
      message="This screen arrives in a later step. The design system (Step 14) is in place; real screens start at Step 15."
    />
  );
}

function Shell() {
  const navigate = useNavigate();
  const location = useLocation();
  const active =
    NAV_ITEMS.find((item) => item.href !== '/' && location.pathname.startsWith(item.href))?.id ??
    'home';
  return (
    <AppShell
      title="ForgeHub"
      navItems={NAV_ITEMS}
      activeId={active}
      onNavigate={(item) => navigate(item.href)}
    >
      <Routes>
        <Route path="/" element={<Placeholder title="Home" />} />
        <Route path="/explore" element={<Placeholder title="Explore" />} />
        <Route path="/library" element={<Placeholder title="Library" />} />
        <Route path="/profile" element={<Placeholder title="Profile" />} />
        {import.meta.env.DEV ? <Route path="/__design" element={<DesignPage />} /> : null}
        <Route path="*" element={<Placeholder title="Not found" />} />
      </Routes>
    </AppShell>
  );
}

export default function App() {
  return (
    <ToastProvider>
      <BrowserRouter>
        <Shell />
      </BrowserRouter>
    </ToastProvider>
  );
}
