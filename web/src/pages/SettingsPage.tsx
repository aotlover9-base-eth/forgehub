import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Button,
  Card,
  CodeBlock,
  ConfirmDialog,
  DialogActions,
  EmptyState,
  ErrorState,
  PageHeader,
  Skeleton,
  Tabs,
  TextField,
  resolveTheme,
  useThemePreference,
  useToast,
  type ThemePreference,
} from '../design';
import { ApiError } from '../api/client';
import {
  useChangePassword,
  useCreateToken,
  useDeleteAccount,
  useLogoutAll,
  useMe,
  usePatchMe,
  useRevokeSession,
  useRevokeToken,
  useSessions,
  useTokens,
} from '../api/hooks';
import type { CreatedToken } from '../api/types';
import { passwordIssue, relativeTime } from '../validation';

const TAB_IDS = ['appearance', 'profile', 'security', 'tokens', 'cli', 'danger'] as const;

export default function SettingsPage() {
  const me = useMe();
  const [tab, setTab] = useState(0);
  if (!me.data) return null;
  const user = me.data;
  return (
    <div style={{ display: 'grid', gap: 'var(--fh-space-4)' }}>
      <PageHeader title="Settings" subtitle={`Signed in as ${user.username}`} />
      <Tabs
        ariaLabel="Settings sections"
        tabs={[
          { id: 'appearance', label: 'Appearance' },
          { id: 'profile', label: 'Profile' },
          { id: 'security', label: 'Security' },
          { id: 'tokens', label: 'Tokens' },
          { id: 'cli', label: 'CLI' },
          { id: 'danger', label: 'Danger zone' },
        ]}
        activeIndex={tab}
        onChange={setTab}
      />
      {TAB_IDS[tab] === 'appearance' ? <AppearanceTab /> : null}
      {TAB_IDS[tab] === 'profile' ? <ProfileTab /> : null}
      {TAB_IDS[tab] === 'security' ? <SecurityTab /> : null}
      {TAB_IDS[tab] === 'tokens' ? <TokensTab /> : null}
      {TAB_IDS[tab] === 'cli' ? <CliTab /> : null}
      {TAB_IDS[tab] === 'danger' ? <DangerTab /> : null}
    </div>
  );
}

const THEME_OPTIONS: { id: ThemePreference; label: string; help: string }[] = [
  { id: 'system', label: 'System', help: 'Follow your device setting.' },
  { id: 'light', label: 'Light', help: 'Always use the light scheme.' },
  { id: 'dark', label: 'Dark', help: 'Always use the dark scheme.' },
];

function AppearanceTab() {
  const [pref, setPref] = useThemePreference();
  return (
    <Card>
      <fieldset style={{ border: 'none', margin: 0, padding: 0 }}>
        <legend style={{ fontWeight: 500, marginBottom: 'var(--fh-space-2)' }}>Theme</legend>
        <div style={{ display: 'grid', gap: 'var(--fh-space-2)' }}>
          {THEME_OPTIONS.map((option) => (
            <label
              key={option.id}
              style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'flex-start', cursor: 'pointer' }}
            >
              <md-radio
                name="theme"
                value={option.id}
                checked={pref === option.id || undefined}
                onChange={() => setPref(option.id)}
              />
              <span>
                <strong>{option.label}</strong> - {option.help}
              </span>
            </label>
          ))}
        </div>
      </fieldset>
      <p>Currently showing the {resolveTheme(pref)} scheme. The same choice lives in the app bar, under Theme.</p>
    </Card>
  );
}

function ProfileTab() {
  const me = useMe();
  const patchMe = usePatchMe();
  const toast = useToast();
  const [displayName, setDisplayName] = useState<string | null>(null);
  const [bio, setBio] = useState<string | null>(null);
  const name = displayName ?? me.data?.display_name ?? '';
  const bioText = bio ?? me.data?.bio ?? '';
  return (
    <div style={{ display: 'grid', gap: 'var(--fh-space-4)', maxWidth: 560 }}>
      <TextField label="Display name" value={name} onChange={setDisplayName} maxLength={80} supportingText="Shown on your profile. At most 80 characters." />
      <TextField label="Bio" value={bioText} onChange={setBio} multiline rows={3} maxLength={280} supportingText="At most 280 characters." />
      <p style={{ margin: 0 }}>
        <Button
          variant="filled"
          disabled={patchMe.isPending}
          onClick={() =>
            patchMe.mutate(
              { display_name: name || null, bio: bioText || null },
              {
                onSuccess: () => toast({ message: 'Profile saved' }),
                onError: (e) => toast({ message: e.uiMessage }),
              },
            )
          }
        >
          Save profile
        </Button>
      </p>
    </div>
  );
}

function SecurityTab() {
  const [current, setCurrent] = useState('');
  const [next, setNext] = useState('');
  const [formError, setFormError] = useState<string | null>(null);
  const changePassword = useChangePassword();
  const sessions = useSessions();
  const revokeSession = useRevokeSession();
  const logoutAll = useLogoutAll();
  const navigate = useNavigate();
  const toast = useToast();
  const me = useMe();
  const nextIssue = passwordIssue(next, me.data?.username ?? '');
  return (
    <div style={{ display: 'grid', gap: 'var(--fh-space-5)', maxWidth: 640 }}>
      <Card variant="outlined">
        <h2 style={{ marginTop: 0 }}>Change password</h2>
        <form
          style={{ display: 'grid', gap: 'var(--fh-space-3)' }}
          onSubmit={(e) => {
            e.preventDefault();
            setFormError(null);
            changePassword.mutate(
              { current_password: current, new_password: next },
              {
                onSuccess: () => {
                  setCurrent('');
                  setNext('');
                  toast({ message: 'Password changed. Other sessions were signed out.' });
                },
                onError: (error) =>
                  setFormError(
                    error instanceof ApiError && error.status === 401
                      ? 'Current password did not match.'
                      : error.uiMessage,
                  ),
              },
            );
          }}
        >
          <TextField label="Current password" value={current} onChange={setCurrent} type="password" autoComplete="current-password" required />
          <TextField
            label="New password"
            value={next}
            onChange={setNext}
            type="password"
            autoComplete="new-password"
            required
            errorText={nextIssue ?? undefined}
            supportingText="At least 10 characters. Changing it signs out every other session."
          />
          {formError ? (
            <p role="alert" style={{ color: 'var(--md-sys-color-error)', margin: 0 }}>
              {formError}
            </p>
          ) : null}
          <p style={{ margin: 0 }}>
            <Button variant="filled" type="submit" disabled={changePassword.isPending || !current || next.length < 10 || !!nextIssue}>
              Change password
            </Button>
          </p>
        </form>
      </Card>

      <Card variant="outlined">
        <h2 style={{ marginTop: 0 }}>Active sessions</h2>
        {sessions.isPending ? (
          <Skeleton lines={2} />
        ) : sessions.isError ? (
          <ErrorState title="Could not load sessions" message="Try again." retryLabel="Retry" onRetry={() => void sessions.refetch()} />
        ) : (
          <>
            <md-list>
              {(sessions.data?.sessions ?? []).map((s) => (
                <md-list-item key={s.ref}>
                  <span slot="headline">
                    {s.current ? 'This browser' : 'Another session'} · last used{' '}
                    <span title={new Date(s.last_used_at).toLocaleString()}>{relativeTime(s.last_used_at)}</span>
                  </span>
                  <span slot="supporting-text">
                    Started {new Date(s.created_at).toLocaleDateString()} · expires{' '}
                    {new Date(s.expires_at).toLocaleDateString()}
                  </span>
                  <span slot="end">
                    <Button
                      variant="text"
                      onClick={() =>
                        revokeSession.mutate(s.ref, {
                          onSuccess: () => {
                            if (s.current) navigate('/login');
                            else toast({ message: 'Session revoked' });
                          },
                          onError: (e) => toast({ message: e.uiMessage }),
                        })
                      }
                    >
                      Revoke
                    </Button>
                  </span>
                </md-list-item>
              ))}
            </md-list>
            <p style={{ margin: 0 }}>
              <Button
                variant="outlined"
                onClick={() =>
                  logoutAll.mutate(undefined, {
                    onSuccess: () => navigate('/'),
                    onError: (e) => toast({ message: e.uiMessage }),
                  })
                }
              >
                Sign out everywhere
              </Button>
            </p>
          </>
        )}
      </Card>
    </div>
  );
}

function TokensTab() {
  const tokens = useTokens();
  const createToken = useCreateToken();
  const revokeToken = useRevokeToken();
  const toast = useToast();
  const [createOpen, setCreateOpen] = useState(false);
  const [tokenName, setTokenName] = useState('');
  const [readScope, setReadScope] = useState(true);
  const [writeScope, setWriteScope] = useState(true);
  const [expiry, setExpiry] = useState('never');
  const [fresh, setFresh] = useState<CreatedToken | null>(null);
  const [copied, setCopied] = useState(false);
  const [revoking, setRevoking] = useState<number | null>(null);

  return (
    <div style={{ display: 'grid', gap: 'var(--fh-space-4)', maxWidth: 720 }}>
      <p style={{ margin: 0 }}>
        Tokens let the CLI and scripts act as you. A token is shown <strong>once</strong>, when it is
        created - after that, only its first characters are visible.
      </p>
      <p style={{ margin: 0 }}>
        <Button variant="filled" icon="add" onClick={() => setCreateOpen(true)}>
          New token
        </Button>
      </p>

      {fresh ? (
        <Card variant="filled">
          <h2 style={{ marginTop: 0 }}>Your new token - copy it now</h2>
          <p>
            This is the only time the full token is shown. Store it somewhere safe; anyone with it
            can act as you within its scopes.
          </p>
          <CodeBlock code={fresh.token} label={fresh.name} />
          <p style={{ display: 'flex', gap: 'var(--fh-space-2)', margin: 0 }}>
            <Button
              variant="tonal"
              onClick={async () => {
                try {
                  await navigator.clipboard.writeText(fresh.token);
                  setCopied(true);
                } catch {
                  toast({ message: 'Copy failed - select the token text and copy it manually' });
                }
              }}
            >
              Copy token
            </Button>
            <Button
              variant="filled"
              disabled={!copied}
              onClick={() => {
                setFresh(null);
                setCopied(false);
              }}
            >
              I've copied it
            </Button>
          </p>
          {!copied ? <p style={{ margin: 0 }}>The button unlocks after you copy the token.</p> : null}
        </Card>
      ) : null}

      {tokens.isPending ? (
        <Skeleton lines={3} />
      ) : tokens.isError ? (
        <ErrorState title="Could not load tokens" message="Try again." retryLabel="Retry" onRetry={() => void tokens.refetch()} />
      ) : (tokens.data?.tokens ?? []).length === 0 ? (
        <EmptyState icon="key" title="No tokens yet" message="Create one for the CLI, or use fg login, which creates one for you." />
      ) : (
        <Card variant="outlined">
          <md-list>
            {(tokens.data?.tokens ?? []).map((t) => (
              <md-list-item key={t.id}>
                <span slot="headline">
                  {t.name} · fgh_{t.prefix}…
                </span>
                <span slot="supporting-text">
                  {t.scopes.join(' + ')} · created {new Date(t.created_at).toLocaleDateString()} ·{' '}
                  {t.revoked_at
                    ? `revoked ${new Date(t.revoked_at).toLocaleDateString()}`
                    : t.last_used_at
                      ? `last used ${relativeTime(t.last_used_at)}`
                      : 'never used'}{' '}
                  · {t.expires_at ? `expires ${new Date(t.expires_at).toLocaleDateString()}` : 'no expiry'}
                </span>
                {!t.revoked_at ? (
                  <span slot="end">
                    <Button variant="text" onClick={() => setRevoking(t.id)}>
                      Revoke
                    </Button>
                  </span>
                ) : null}
              </md-list-item>
            ))}
          </md-list>
        </Card>
      )}

      <md-dialog open={createOpen || undefined} onClose={() => setCreateOpen(false)}>
        <div slot="headline">New token</div>
        <div slot="content" style={{ display: 'grid', gap: 'var(--fh-space-3)', minWidth: 320 }}>
          <TextField label="Name" value={tokenName} onChange={setTokenName} supportingText="What is this token for? e.g. Laptop CLI" />
          <label style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center' }}>
            <md-checkbox checked={readScope || undefined} onChange={() => setReadScope(!readScope)} /> Read - view
            and run snippets
          </label>
          <label style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center' }}>
            <md-checkbox checked={writeScope || undefined} onChange={() => setWriteScope(!writeScope)} /> Write -
            create and change snippets (includes read)
          </label>
          <label style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center' }}>
            Expires
            <select value={expiry} onChange={(e) => setExpiry(e.target.value)} aria-label="Expiry">
              <option value="never">Never</option>
              <option value="30">In 30 days</option>
              <option value="90">In 90 days</option>
              <option value="365">In 365 days</option>
            </select>
          </label>
        </div>
        <div slot="actions">
          <DialogActions
            confirmLabel="Create token"
            confirmDisabled={tokenName.trim() === '' || (!readScope && !writeScope)}
            onCancel={() => setCreateOpen(false)}
            onConfirm={() =>
              createToken.mutate(
                {
                  name: tokenName.trim(),
                  scopes: writeScope ? ['read', 'write'] : ['read'],
                  expires_in_days: expiry === 'never' ? undefined : Number(expiry),
                },
                {
                  onSuccess: (created) => {
                    setFresh(created);
                    setCreateOpen(false);
                    setTokenName('');
                  },
                  onError: (e) => toast({ message: e.uiMessage }),
                },
              )
            }
          />
        </div>
      </md-dialog>

      <ConfirmDialog
        open={revoking !== null}
        headline="Revoke this token?"
        content="Anything using it will stop working immediately. This cannot be undone."
        confirmLabel="Revoke"
        danger
        onCancel={() => setRevoking(null)}
        onConfirm={() => {
          if (revoking === null) return;
          revokeToken.mutate(revoking, {
            onSuccess: () => toast({ message: 'Token revoked' }),
            onError: (e) => toast({ message: e.uiMessage }),
            onSettled: () => setRevoking(null),
          });
        }}
      />
    </div>
  );
}

function CliTab() {
  return (
    <div style={{ display: 'grid', gap: 'var(--fh-space-4)', maxWidth: 640 }}>
      <Card variant="outlined">
        <h2 style={{ marginTop: 0 }}>The fg CLI</h2>
        <p>
          The CLI runs your snippets from any terminal. Install instructions are in the ForgeHub
          README (installer packages arrive with the first release).
        </p>
        <p>Sign in once - your browser approves it on the device page:</p>
        <CodeBlock label="Terminal" code={'fg login\nfg ubuntu-setup'} />
        <p style={{ margin: 0 }}>
          <code>fg login</code> shows a short code and opens <code>/device</code>. Nothing is granted
          until you approve it there.
        </p>
      </Card>
    </div>
  );
}

function DangerTab() {
  const me = useMe();
  const deleteAccount = useDeleteAccount();
  const navigate = useNavigate();
  const toast = useToast();
  const [open, setOpen] = useState(false);
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const myName = me.data?.username ?? '';
  return (
    <div style={{ display: 'grid', gap: 'var(--fh-space-4)', maxWidth: 560 }}>
      <Card variant="outlined">
        <h2 style={{ marginTop: 0 }}>Delete account</h2>
        <p>
          Your account is disabled immediately and your snippets, tokens and sessions are revoked.
          After 30 days everything is purged for good. This cannot be undone after the purge.
        </p>
        <Button variant="outlined" onClick={() => setOpen(true)}>
          Delete my account
        </Button>
      </Card>
      <md-dialog open={open || undefined} onClose={() => setOpen(false)}>
        <div slot="headline">Delete account?</div>
        <div slot="content" style={{ display: 'grid', gap: 'var(--fh-space-3)', minWidth: 320 }}>
          <p style={{ margin: 0 }}>
            Type your username <strong>{myName}</strong> and your password to confirm.
          </p>
          <TextField label={`Type "${myName}"`} value={username} onChange={setUsername} />
          <TextField label="Password" value={password} onChange={setPassword} type="password" autoComplete="current-password" />
          {error ? (
            <p role="alert" style={{ color: 'var(--md-sys-color-error)', margin: 0 }}>
              {error}
            </p>
          ) : null}
        </div>
        <div slot="actions">
          <DialogActions
            confirmLabel="Delete account"
            danger
            confirmDisabled={username !== myName || password === ''}
            onCancel={() => setOpen(false)}
            onConfirm={() => {
              setError(null);
              deleteAccount.mutate(
                { username, password },
                {
                  onSuccess: () => {
                    toast({ message: 'Account scheduled for deletion. You are signed out.' });
                    navigate('/');
                  },
                  onError: (e) =>
                    setError(
                      e instanceof ApiError && e.status === 401
                        ? 'Password did not match.'
                        : e.uiMessage,
                    ),
                },
              );
            }}
          />
        </div>
      </md-dialog>
    </div>
  );
}
