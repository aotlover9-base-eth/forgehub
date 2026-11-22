import { useState } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router-dom';
import { Button, Card, IconButton, PageHeader, TextField, useToast } from '../design';
import { ApiError } from '../api/client';
import { m } from '../i18n/messages';
import { useLogin } from '../api/hooks';

export default function LoginPage() {
  const [identifier, setIdentifier] = useState('');
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const login = useLogin();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const toast = useToast();

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    setFormError(null);
    login.mutate(
      { identifier: identifier.trim(), password },
      {
        onSuccess: () => {
          toast({ message: m.auth.loginWelcomeToast });
          navigate(params.get('return') || '/home', { replace: true });
        },
        onError: (error) => {
          // Login failures are generic by design (no account
          // oracle); rate limits get the wait message.
          setFormError(
            error instanceof ApiError && error.status === 429
              ? error.uiMessage
              : m.auth.loginFailed,
          );
        },
      },
    );
  };

  return (
    <div style={{ maxWidth: 440, margin: '0 auto' }}>
      <PageHeader title={m.auth.loginTitle} subtitle={m.auth.loginSubtitle} />
      <Card variant="outlined">
        <form onSubmit={submit} style={{ display: 'grid', gap: 'var(--fh-space-4)' }}>
          <TextField
            label={m.auth.loginIdentifier}
            value={identifier}
            onChange={setIdentifier}
            autoComplete="username"
            required
          />
          <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'flex-start' }}>
            <div style={{ flex: 1 }}>
              <TextField
                label={m.common.password}
                value={password}
                onChange={setPassword}
                type={showPassword ? 'text' : 'password'}
                autoComplete="current-password"
                required
              />
            </div>
            <IconButton
              icon={showPassword ? 'visibility_off' : 'visibility'}
              label={showPassword ? m.common.hidePassword : m.common.showPassword}
              onClick={() => setShowPassword(!showPassword)}
            />
          </div>
          {formError ? (
            <p role="alert" style={{ color: 'var(--md-sys-color-error)', margin: 0 }}>
              {formError}
            </p>
          ) : null}
          <Button variant="filled" type="submit" disabled={login.isPending || !identifier || !password}>
            {login.isPending ? m.auth.loginPending : m.auth.loginSubmit}
          </Button>
        </form>
      </Card>
      <p>
        <Link to="/forgot-password">{m.auth.loginForgotLink}</Link>
      </p>
      <p>
        {m.auth.loginNewTo} <Link to="/signup">{m.auth.loginCreateLink}</Link>
      </p>
    </div>
  );
}
