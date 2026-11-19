import { Link } from 'react-router-dom';
import { Button, CodeBlock } from '../design';
import { useMe } from '../api/hooks';

/**
 * The home page for everyone: one calm hero — what ForgeHub
 * is and the canonical example. Signed-out visitors get the
 * way in; signed-in users get the way to their library.
 * Home never silently becomes another page (it used to
 * redirect signed-in users to the dashboard, which the nav
 * calls Library — "Home goes to Library" was a real
 * complaint).
 */
export default function LandingPage() {
  const me = useMe();
  return (
    <div style={{ maxWidth: 720, margin: '0 auto', display: 'grid', gap: 'var(--fh-space-5)' }}>
      <div>
        <h1
          style={{
            fontSize: 'var(--md-sys-typescale-display-small-size)',
            lineHeight: 'var(--md-sys-typescale-display-small-line-height)',
            fontWeight: 500,
            margin: 'var(--fh-space-4) 0 var(--fh-space-2)',
          }}
        >
          Save a command once.
          <br />
          Run it anywhere.
        </h1>
        <p
          style={{
            color: 'var(--md-sys-color-on-surface-variant)',
            fontSize: 'var(--md-sys-typescale-body-large-size)',
            maxWidth: 560,
          }}
        >
          ForgeHub is a home for the shell commands you keep re-typing. Store them once, then run
          them from any machine with <code>fg</code> — after seeing exactly what will run and
          confirming it.
        </p>
      </div>
      <CodeBlock
        label="From any machine, after fg login"
        code={'# someone saved this once as "ubuntu-setup"\nfg ubuntu-setup'}
      />
      <p style={{ display: 'flex', gap: 'var(--fh-space-3)', alignItems: 'center', margin: 0 }}>
        {me.data ? (
          <>
            <Link to="/dashboard" style={{ textDecoration: 'none' }}>
              <Button variant="filled">Open your library</Button>
            </Link>
            <Link to="/explore" style={{ textDecoration: 'none' }}>
              <Button variant="text">Explore</Button>
            </Link>
          </>
        ) : (
          <>
            <Link to="/signup" style={{ textDecoration: 'none' }}>
              <Button variant="filled">Create account</Button>
            </Link>
            <Link to="/explore" style={{ textDecoration: 'none' }}>
              <Button variant="text">Explore</Button>
            </Link>
          </>
        )}
      </p>
    </div>
  );
}
