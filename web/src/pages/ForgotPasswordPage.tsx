import { useState } from 'react';
import { Link } from 'react-router-dom';
import { Button, Card, PageHeader, TextField } from '../design';
import { usePasswordResetRequest } from '../api/hooks';
import { m } from '../i18n/messages';

/**
 * Ask for a reset link. The confirmation state is shown for
 * every submitted address - the server answers identically
 * whether or not the account exists, and this page must not
 * become an account oracle either.
 */
export default function ForgotPasswordPage() {
  const [email, setEmail] = useState('');
  const [sentTo, setSentTo] = useState<string | null>(null);
  const request = usePasswordResetRequest();

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    const target = email.trim();
    request.mutate(
      { email: target },
      {
        // Even a transport error lands on the same message:
        // the user learns nothing about the address either
        // way, and can simply try again later.
        onSuccess: () => setSentTo(target),
        onError: () => setSentTo(target),
      },
    );
  };

  return (
    <div style={{ maxWidth: 440, margin: '0 auto' }}>
      <PageHeader
        title={m.auth.forgotTitle}
        subtitle={m.auth.forgotSubtitle}
      />
      <Card variant="outlined">
        {sentTo ? (
          <div style={{ display: 'grid', gap: 'var(--fh-space-3)' }}>
            <p style={{ margin: 0 }}>
              {m.auth.forgotSentBefore} <strong>{sentTo}</strong>{m.auth.forgotSentAfter}
            </p>
            <p style={{ margin: 0 }}>
              {m.auth.forgotNoEmailBefore} <Link to="/login">{m.auth.forgotNoEmailLogin}</Link> {m.auth.forgotNoEmailAfter}
            </p>
          </div>
        ) : (
          <form onSubmit={submit} style={{ display: 'grid', gap: 'var(--fh-space-4)' }}>
            <TextField
              label={m.auth.forgotEmail}
              value={email}
              onChange={setEmail}
              type="email"
              autoComplete="email"
              required
            />
            <Button variant="filled" type="submit" disabled={request.isPending || !email.trim()}>
              {request.isPending ? m.auth.forgotPending : m.auth.forgotSubmit}
            </Button>
          </form>
        )}
      </Card>
      <p>
        {m.auth.forgotRemembered} <Link to="/login">{m.auth.forgotLoginLink}</Link>
      </p>
    </div>
  );
}
