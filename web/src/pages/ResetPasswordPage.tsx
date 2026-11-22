import { useState } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router-dom';
import { Button, Card, IconButton, PageHeader, TextField, useToast } from '../design';
import { ApiError } from '../api/client';
import { m } from '../i18n/messages';
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
        <PageHeader title={m.auth.resetMissingTitle} />
        <Card variant="outlined">
          <p style={{ margin: 0 }}>
            {m.auth.resetMissingBefore}{' '}
            <Link to="/forgot-password">{m.auth.resetMissingLink}</Link>.
          </p>
        </Card>
      </div>
    );
  }

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    setFormError(null);
    if (password !== confirm) {
      setFormError(m.auth.resetMismatch);
      return;
    }
    reset.mutate(
      { token, new_password: password },
      {
        onSuccess: () => {
          toast({ message: m.auth.resetUpdatedToast });
          navigate('/login', { replace: true });
        },
        onError: (error) => {
          setFormError(
            error instanceof ApiError
              ? error.uiMessage
              : m.auth.resetFailed,
          );
        },
      },
    );
  };

  return (
    <div style={{ maxWidth: 440, margin: '0 auto' }}>
      <PageHeader
        title={m.auth.resetTitle}
        subtitle={m.auth.resetSubtitle}
      />
      <Card variant="outlined">
        <form onSubmit={submit} style={{ display: 'grid', gap: 'var(--fh-space-4)' }}>
          <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'flex-start' }}>
            <div style={{ flex: 1 }}>
              <TextField
                label={m.auth.resetNewPassword}
                value={password}
                onChange={setPassword}
                type={showPassword ? 'text' : 'password'}
                autoComplete="new-password"
                required
              />
            </div>
            <IconButton
              icon={showPassword ? 'visibility_off' : 'visibility'}
              label={showPassword ? m.common.hidePassword : m.common.showPassword}
              onClick={() => setShowPassword(!showPassword)}
            />
          </div>
          <TextField
            label={m.auth.resetConfirmPassword}
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
            {reset.isPending ? m.auth.resetPending : m.auth.resetSubmit}
          </Button>
        </form>
      </Card>
      <p>
        {m.auth.resetExpiredBefore} <Link to="/forgot-password">{m.auth.resetRequestNew}</Link>
      </p>
    </div>
  );
}
