import { useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { Button, Card, IconButton, PageHeader, TextField, useToast } from '../design';
import { ApiError } from '../api/client';
import { m } from '../i18n/messages';
import { useSignup } from '../api/hooks';
import { emailIssue, passwordIssue, usernameIssue } from '../validation';

export default function SignupPage() {
  const [username, setUsername] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [serverUsernameError, setServerUsernameError] = useState<string | null>(null);
  const [formError, setFormError] = useState<string | null>(null);
  const signup = useSignup();
  const navigate = useNavigate();
  const toast = useToast();

  const usernameHint = serverUsernameError ?? usernameIssue(username);
  const emailHint = emailIssue(email);
  const passwordHint = passwordIssue(password, username);
  const valid =
    username.length >= 2 &&
    !usernameIssue(username) &&
    email.length > 3 &&
    !emailHint &&
    password.length >= 10 &&
    !passwordHint;

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    setFormError(null);
    setServerUsernameError(null);
    signup.mutate(
      { username: username.trim(), email: email.trim(), password },
      {
        onSuccess: () => {
          toast({ message: m.auth.signupCreatedToast });
          navigate('/home', { replace: true });
        },
        onError: (error) => {
          if (error instanceof ApiError && error.code === 'username_taken') {
            setServerUsernameError(m.auth.signupUsernameTaken);
          } else if (error instanceof ApiError && error.status === 429) {
            setFormError(error.uiMessage);
          } else if (error instanceof ApiError && error.status === 409) {
            // Generic by design: the server does not confirm
            // whether the email already has an account.
            setFormError(m.auth.signupCouldNotCreate);
          } else if (error instanceof ApiError) {
            setFormError(error.uiMessage);
          } else {
            setFormError(m.common.somethingWrong);
          }
        },
      },
    );
  };

  return (
    <div style={{ maxWidth: 440, margin: '0 auto' }}>
      <PageHeader title={m.auth.signupTitle} subtitle={m.auth.signupSubtitle} />
      <Card variant="outlined">
        <form onSubmit={submit} style={{ display: 'grid', gap: 'var(--fh-space-4)' }}>
          <TextField
            label={m.auth.signupUsername}
            value={username}
            onChange={(v) => {
              setUsername(v);
              setServerUsernameError(null);
            }}
            autoComplete="username"
            required
            errorText={usernameHint ?? undefined}
            supportingText={m.auth.signupUsernameHint}
          />
          <TextField
            label={m.auth.signupEmail}
            value={email}
            onChange={setEmail}
            type="email"
            autoComplete="email"
            required
            errorText={emailHint ?? undefined}
          />
          <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'flex-start' }}>
            <div style={{ flex: 1 }}>
              <TextField
                label={m.common.password}
                value={password}
                onChange={setPassword}
                type={showPassword ? 'text' : 'password'}
                autoComplete="new-password"
                required
                errorText={passwordHint ?? undefined}
                supportingText={m.auth.signupPasswordHint}
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
          <Button variant="filled" type="submit" disabled={signup.isPending || !valid}>
            {signup.isPending ? m.auth.signupPending : m.auth.signupSubmit}
          </Button>
        </form>
      </Card>
      <p>
        {m.auth.signupHaveAccount} <Link to="/login">{m.auth.signupLoginLink}</Link>
      </p>
    </div>
  );
}
