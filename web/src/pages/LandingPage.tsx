import { Link, Navigate } from 'react-router-dom';
import { Button, CodeBlock } from '../design';
import { useMe } from '../api/hooks';

/**
 * The logged-out home: one calm hero — what ForgeHub is, the
 * canonical example, and the two ways in.
 */
export default function LandingPage() {
  const me = useMe();
  if (me.data) {
    return <Navigate to="/dashboard" replace />;
  }
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
        <Link to="/signup" style={{ textDecoration: 'none' }}>
          <Button variant="filled">Create account</Button>
        </Link>
        <Link to="/explore" style={{ textDecoration: 'none' }}>
          <Button variant="text">Explore</Button>
        </Link>
      </p>
    </div>
  );
}
