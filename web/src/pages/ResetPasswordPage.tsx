import { useState } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router-dom';
import { Button, Card, IconButton, PageHeader, TextField, useToast } from '../design';
import { ApiError } from '../api/client';
import { usePasswordResetConfirm } from '../api/hooks';

/**
 * Land here from the emailed link (?token=…). A successful
 * reset revokes every session and API token on the account
 * server-side, so the only way forward is a fresh login with
 * the new password.
 */
export default function ResetPasswordPage() {
  const [params] = useSearchParams();
  const token = params.get('token') ?? '';
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const reset = usePasswordResetConfirm();
  const navigate = useNavigate();
  const toast = useToast();

  if (!token) {
    return (
      <div style={{ maxWidth: 440, margin: '0 auto' }}>
        <PageHeader title="Reset your password" />
        <Card variant="outlined">
          <p style={{ margin: 0 }}>
            This link is missing its token. Open the link from your email again, or{' '}
            <Link to="/forgot-password">request a new one</Link>.
          </p>
        </Card>
      </div>
    );
  }

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    setFormError(null);
    if (password !== confirm) {
      setFormError('The two passwords do not match.');
      return;
    }
    reset.mutate(
      { token, new_password: password },
      {
        onSuccess: () => {
          toast({ message: 'Password updated - log in with your new password' });
          navigate('/login', { replace: true });
        },
        onError: (error) => {
          setFormError(
            error instanceof ApiError
              ? error.uiMessage
              : 'Something went wrong. Request a new link and try again.',
          );
        },
      },
    );
  };

  return (
    <div style={{ maxWidth: 440, margin: '0 auto' }}>
      <PageHeader
        title="Set a new password"
        subtitle="Pick a new password. Everywhere you are signed in will be signed out."
      />
      <Card variant="outlined">
        <form onSubmit={submit} style={{ display: 'grid', gap: 'var(--fh-space-4)' }}>
          <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'flex-start' }}>
            <div style={{ flex: 1 }}>
              <TextField
                label="New password"
                value={password}
                onChange={setPassword}
                type={showPassword ? 'text' : 'password'}
                autoComplete="new-password"
                required
              />
            </div>
            <IconButton
              icon={showPassword ? 'visibility_off' : 'visibility'}
              label={showPassword ? 'Hide password' : 'Show password'}
              onClick={() => setShowPassword(!showPassword)}
            />
          </div>
          <TextField
            label="Confirm new password"
            value={confirm}
            onChange={setConfirm}
            type={showPassword ? 'text' : 'password'}
            autoComplete="new-password"
            required
          />
          {formError ? (
            <p role="alert" style={{ color: 'var(--md-sys-color-error)', margin: 0 }}>
              {formError}
            </p>
          ) : null}
          <Button
            variant="filled"
            type="submit"
            disabled={reset.isPending || !password || !confirm}
          >
            {reset.isPending ? 'Updating…' : 'Update password'}
          </Button>
        </form>
      </Card>
      <p>
        Link expired? <Link to="/forgot-password">Request a new one</Link>
      </p>
    </div>
  );
}
