import { useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { Button, Card, IconButton, PageHeader, TextField, useToast } from '../design';
import { ApiError } from '../api/client';
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
          toast({ message: 'Account created. Welcome to ForgeHub.' });
          navigate('/dashboard', { replace: true });
        },
        onError: (error) => {
          if (error instanceof ApiError && error.code === 'username_taken') {
            setServerUsernameError('That username is taken. Try another.');
          } else if (error instanceof ApiError && error.status === 429) {
            setFormError(error.uiMessage);
          } else if (error instanceof ApiError && error.status === 409) {
            // Generic by design: the server does not confirm
            // whether the email already has an account.
            setFormError('We could not create the account with those details. If you already have an account, log in instead.');
          } else if (error instanceof ApiError) {
            setFormError(error.uiMessage);
          } else {
            setFormError('Something went wrong. Try again.');
          }
        },
      },
    );
  };

  return (
    <div style={{ maxWidth: 440, margin: '0 auto' }}>
      <PageHeader title="Create account" subtitle="Save a command once. Run it anywhere with fg." />
      <Card variant="outlined">
        <form onSubmit={submit} style={{ display: 'grid', gap: 'var(--fh-space-4)' }}>
          <TextField
            label="Username"
            value={username}
            onChange={(v) => {
              setUsername(v);
              setServerUsernameError(null);
            }}
            autoComplete="username"
            required
            errorText={usernameHint ?? undefined}
            supportingText="Lowercase letters, digits and hyphens. This is your public name in URLs."
          />
          <TextField
            label="Email"
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
                label="Password"
                value={password}
                onChange={setPassword}
                type={showPassword ? 'text' : 'password'}
                autoComplete="new-password"
                required
                errorText={passwordHint ?? undefined}
                supportingText="At least 10 characters. A longer phrase you remember beats a short clever one."
              />
            </div>
            <IconButton
              icon={showPassword ? 'visibility_off' : 'visibility'}
              label={showPassword ? 'Hide password' : 'Show password'}
              onClick={() => setShowPassword(!showPassword)}
            />
          </div>
          {formError ? (
            <p role="alert" style={{ color: 'var(--md-sys-color-error)', margin: 0 }}>
              {formError}
            </p>
          ) : null}
          <Button variant="filled" type="submit" disabled={signup.isPending || !valid}>
            {signup.isPending ? 'Creating account…' : 'Create account'}
          </Button>
        </form>
      </Card>
      <p>
        Already have an account? <Link to="/login">Log in</Link>
      </p>
    </div>
  );
}
