import { useState } from 'react';
import { Link } from 'react-router-dom';
import { Button, Card, PageHeader, TextField } from '../design';
import { usePasswordResetRequest } from '../api/hooks';

/**
 * Ask for a reset link. The confirmation state is shown for
 * every submitted address — the server answers identically
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
        title="Forgot your password?"
        subtitle="Enter the email address on your account and we will send you a reset link."
      />
      <Card variant="outlined">
        {sentTo ? (
          <div style={{ display: 'grid', gap: 'var(--fh-space-3)' }}>
            <p style={{ margin: 0 }}>
              If an account exists for <strong>{sentTo}</strong>, a reset link is on its way. It
              expires in one hour.
            </p>
            <p style={{ margin: 0 }}>
              No email after a few minutes? Check spam, then <Link to="/login">log in</Link> and
              try again.
            </p>
          </div>
        ) : (
          <form onSubmit={submit} style={{ display: 'grid', gap: 'var(--fh-space-4)' }}>
            <TextField
              label="Email"
              value={email}
              onChange={setEmail}
              type="email"
              autoComplete="email"
              required
            />
            <Button variant="filled" type="submit" disabled={request.isPending || !email.trim()}>
              {request.isPending ? 'Sending…' : 'Send reset link'}
            </Button>
          </form>
        )}
      </Card>
      <p>
        Remembered it after all? <Link to="/login">Log in</Link>
      </p>
    </div>
  );
}
