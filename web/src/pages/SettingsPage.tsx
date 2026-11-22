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
  Checkbox,
  PageHeader,
  Radio,
  SelectField,
  Skeleton,
  Switch,
  Tabs,
  TextField,
  resolveTheme,
  useDensity,
  useThemePreference,
  useToast,
  useWidthClass,
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
  useActivity,
  useSessions,
  useTokens,
} from '../api/hooks';
import type { CreatedToken } from '../api/types';
import { passwordIssue, relativeTime } from '../validation';
import { fmt, m } from '../i18n/messages';

const TAB_IDS = ['appearance', 'profile', 'security', 'tokens', 'cli', 'danger'] as const;

const SECTION_LABELS: Record<string, string> = {
  appearance: m.settings.sectionAppearance,
  profile: m.settings.sectionProfile,
  security: m.settings.sectionSecurity,
  tokens: m.settings.sectionTokens,
  cli: m.settings.sectionCli,
  danger: m.settings.sectionDanger,
};

function sectionElement(id: string) {
  switch (id) {
    case 'appearance':
      return <AppearanceTab />;
    case 'profile':
      return <ProfileTab />;
    case 'security':
      return <SecurityTab />;
    case 'tokens':
      return <TokensTab />;
    case 'cli':
      return <CliTab />;
    default:
      return <DangerTab />;
  }
}

export default function SettingsPage() {
  const me = useMe();
  const [tab, setTab] = useState(0);
  const widthClass = useWidthClass();
  if (!me.data) return null;
  const user = me.data;
  const activeId = TAB_IDS[tab];
  return (
    <div style={{ display: 'grid', gap: 'var(--fh-space-4)' }}>
      <PageHeader title={m.settings.title} subtitle={fmt(m.settings.signedInAs, { username: user.username })} />
      {widthClass === 'compact' ? (
        // Compact: every section stacked with its own head.
        <div style={{ display: 'grid', gap: 'var(--fh-space-5)' }}>
          {TAB_IDS.map((id) => (
            <section key={id} aria-label={SECTION_LABELS[id]}>
              <h2 className="fh-settings-section-title">{SECTION_LABELS[id]}</h2>
              {sectionElement(id)}
            </section>
          ))}
        </div>
      ) : widthClass === 'expanded' ? (
        // Expanded: vertical tab rail on the left.
        <div style={{ display: 'flex', gap: 'var(--fh-space-5)', alignItems: 'flex-start' }}>
          <nav className="fh-settings-nav" aria-label={m.settings.sectionsAria}>
            {TAB_IDS.map((id, index) => (
              <button
                key={id}
                type="button"
                aria-current={index === tab ? 'page' : undefined}
                className={`fh-settings-nav__item${index === tab ? ' fh-settings-nav__item--active' : ''}`}
                onClick={() => setTab(index)}
              >
                {SECTION_LABELS[id]}
              </button>
            ))}
          </nav>
          <div style={{ flex: 1, minWidth: 0 }}>{sectionElement(activeId)}</div>
        </div>
      ) : (
        <>
          <Tabs
            ariaLabel={m.settings.sectionsAria}
            tabs={TAB_IDS.map((id) => ({ id, label: SECTION_LABELS[id] }))}
            activeIndex={tab}
            onChange={setTab}
          />
          {sectionElement(activeId)}
        </>
      )}
    </div>
  );
}

const THEME_OPTIONS: { id: ThemePreference; label: string; help: string }[] = [
  { id: 'system', label: m.settings.themeSystem, help: m.settings.themeSystemHelp },
  { id: 'light', label: m.settings.themeLight, help: m.settings.themeLightHelp },
  { id: 'dark', label: m.settings.themeDark, help: m.settings.themeDarkHelp },
];

function AppearanceTab() {
  const [pref, setPref] = useThemePreference();
  const [density, setDensity] = useDensity();
  return (
    <Card>
      <fieldset style={{ border: 'none', margin: 0, padding: 0 }}>
        <legend style={{ fontWeight: 500, marginBottom: 'var(--fh-space-2)' }}>{m.settings.theme}</legend>
        <div style={{ display: 'grid', gap: 'var(--fh-space-2)' }}>
          {THEME_OPTIONS.map((option) => (
            <Radio
              key={option.id}
              name="theme"
              value={option.id}
              checked={pref === option.id}
              onChange={(value) => setPref(value as ThemePreference)}
              label={
                <>
                  <strong>{option.label}</strong> - {option.help}
                </>
              }
            />
          ))}
        </div>
      </fieldset>
      <p>{fmt(m.settings.currentlyShowing, { scheme: resolveTheme(pref) })}</p>
      <div>
        <p style={{ fontWeight: 500, margin: '0 0 var(--fh-space-2)' }}>{m.settings.density}</p>
        <Switch
          checked={density === 'compact'}
          onChange={(compact) => setDensity(compact ? 'compact' : 'comfortable')}
          label={m.settings.densityLabel}
        />
        <p style={{ color: 'var(--md-sys-color-on-surface-variant)', margin: 'var(--fh-space-1) 0 0' }}>
          {m.settings.densityHelp}
        </p>
      </div>
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
      <TextField label={m.settings.displayName} value={name} onChange={setDisplayName} maxLength={80} supportingText={m.settings.displayNameHint} />
      <TextField label={m.settings.bio} value={bioText} onChange={setBio} multiline rows={3} maxLength={280} supportingText={m.settings.bioHint} />
      <p style={{ margin: 0 }}>
        <Button
          variant="filled"
          disabled={patchMe.isPending}
          onClick={() =>
            patchMe.mutate(
              { display_name: name || null, bio: bioText || null },
              {
                onSuccess: () => toast({ message: m.settings.profileSaved }),
                onError: (e) => toast({ message: e.uiMessage }),
              },
            )
          }
        >
          {m.settings.saveProfile}
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
        <h2 style={{ marginTop: 0 }}>{m.settings.changePassword}</h2>
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
                  toast({ message: m.settings.passwordChanged });
                },
                onError: (error) =>
                  setFormError(
                    error instanceof ApiError && error.status === 401
                      ? m.settings.currentMismatch
                      : error.uiMessage,
                  ),
              },
            );
          }}
        >
          <TextField label={m.settings.currentPassword} value={current} onChange={setCurrent} type="password" autoComplete="current-password" required />
          <TextField
            label={m.settings.newPassword}
            value={next}
            onChange={setNext}
            type="password"
            autoComplete="new-password"
            required
            errorText={nextIssue ?? undefined}
            supportingText={m.settings.newPasswordHint}
          />
          {formError ? (
            <p role="alert" style={{ color: 'var(--md-sys-color-error)', margin: 0 }}>
              {formError}
            </p>
          ) : null}
          <p style={{ margin: 0 }}>
            <Button variant="filled" type="submit" disabled={changePassword.isPending || !current || next.length < 10 || !!nextIssue}>
              {m.settings.changePassword}
            </Button>
          </p>
        </form>
      </Card>

      <Card variant="outlined">
        <h2 style={{ marginTop: 0 }}>{m.settings.activeSessions}</h2>
        {sessions.isPending ? (
          <Skeleton lines={2} />
        ) : sessions.isError ? (
          <ErrorState title={m.settings.sessionsFailed} message={m.settings.tryAgain} retryLabel={m.common.retry} onRetry={() => void sessions.refetch()} />
        ) : (
          <>
            <md-list>
              {(sessions.data?.sessions ?? []).map((s) => (
                <md-list-item key={s.ref}>
                  <span slot="headline">
                    {s.current ? m.settings.thisBrowser : m.settings.anotherSession} · {m.settings.lastUsedWord}{' '}
                    <span title={new Date(s.last_used_at).toLocaleString()}>{relativeTime(s.last_used_at)}</span>
                  </span>
                  <span slot="supporting-text">
                    {fmt(m.settings.sessionLine, { started: new Date(s.created_at).toLocaleDateString(), expires: new Date(s.expires_at).toLocaleDateString() })}
                  </span>
                  <span slot="end">
                    <Button
                      variant="text"
                      onClick={() =>
                        revokeSession.mutate(s.ref, {
                          onSuccess: () => {
                            if (s.current) navigate('/login');
                            else toast({ message: m.settings.sessionRevoked });
                          },
                          onError: (e) => toast({ message: e.uiMessage }),
                        })
                      }
                    >
                      {m.settings.revoke}
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
                {m.settings.signOutEverywhere}
              </Button>
            </p>
          </>
        )}
      </Card>
      <ActivityCard />
    </div>
  );
}

/** Plain-language rendering of one activity event. */
function describeEvent(kind: string, detail: Record<string, unknown>): string {
  const s = (key: string) => (typeof detail[key] === 'string' ? (detail[key] as string) : '');
  switch (kind) {
    case 'login':
      return `${fmt(m.settings.actLogin, { via: s('via') || 'login' })}${detail.new_device === true ? m.settings.actLoginNewDevice : ''}${s('ip') ? fmt(m.settings.actLoginIp, { ip: s('ip') }) : ''}`;
    case 'snippet_created':
      return fmt(m.settings.actCreated, { name: s('name') });
    case 'snippet_pushed':
      return fmt(m.settings.actPushed, { name: s('name'), version: String(detail.version ?? '') });
    case 'snippet_forked':
      return fmt(m.settings.actForked, { from: s('from'), name: s('name') });
    case 'token_created':
      return fmt(m.settings.actTokenCreated, { name: s('name') });
    case 'token_revoked':
      return fmt(m.settings.actTokenRevoked, { prefix: s('prefix') });
    case 'snippet_trusted':
      return fmt(m.settings.actTrusted, { ref: s('ref'), version: String(detail.version ?? '') });
    default:
      return kind;
  }
}

/**
 * The activity log (Peak C7): sign-ins, snippet and token
 * events and trust grants for this account, with a JSON
 * export. Only the owner ever sees this - the endpoint is
 * session/token-scoped to the caller.
 */
function ActivityCard() {
  const activity = useActivity(50);
  return (
    <Card variant="outlined">
      <h2 style={{ marginTop: 0 }}>{m.settings.activityTitle}</h2>
      <p style={{ marginTop: 0 }}>
        {m.settings.activityIntro}
      </p>
      {activity.isPending ? (
        <Skeleton />
      ) : activity.isError ? (
        <ErrorState
          title={m.settings.activityFailed}
          message={m.settings.activityFailedMessage}
          onRetry={() => void activity.refetch()}
        />
      ) : activity.data.events.length === 0 ? (
        <p style={{ margin: 0 }}>{m.settings.activityEmpty}</p>
      ) : (
        <md-list>
          {activity.data.events.map((event) => (
            <md-list-item key={event.id}>
              <span slot="headline">{describeEvent(event.kind, event.detail)}</span>
              <span slot="supporting-text">{event.created_at}</span>
            </md-list-item>
          ))}
        </md-list>
      )}
      <p style={{ margin: 0 }}>
        <Button
          variant="outlined"
          icon="download"
          onClick={() => {
            window.location.href = '/api/v1/me/activity/export';
          }}
        >
          {m.settings.exportJson}
        </Button>
      </p>
    </Card>
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
        {m.settings.tokensIntroBefore} <strong>{m.settings.tokensIntroStrong}</strong>{m.settings.tokensIntroAfter}
      </p>
      <p style={{ margin: 0 }}>
        <Button variant="filled" icon="add" onClick={() => setCreateOpen(true)}>
          {m.settings.newToken}
        </Button>
      </p>

      {fresh ? (
        <Card variant="filled">
          <h2 style={{ marginTop: 0 }}>{m.settings.freshTitle}</h2>
          <p>
            {m.settings.freshBody}
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
                  toast({ message: m.settings.copyFailedManual });
                }
              }}
            >
              {m.settings.copyToken}
            </Button>
            <Button
              variant="filled"
              disabled={!copied}
              onClick={() => {
                setFresh(null);
                setCopied(false);
              }}
            >
              {m.settings.copiedIt}
            </Button>
          </p>
          {!copied ? <p style={{ margin: 0 }}>{m.settings.unlockNote}</p> : null}
        </Card>
      ) : null}

      {tokens.isPending ? (
        <Skeleton lines={3} />
      ) : tokens.isError ? (
        <ErrorState title={m.settings.tokensFailed} message={m.settings.tryAgain} retryLabel={m.common.retry} onRetry={() => void tokens.refetch()} />
      ) : (tokens.data?.tokens ?? []).length === 0 ? (
        <EmptyState icon="key" title={m.settings.noTokensTitle} message={m.settings.noTokensMessage} />
      ) : (
        <Card variant="outlined">
          <md-list>
            {(tokens.data?.tokens ?? []).map((t) => (
              <md-list-item key={t.id}>
                <span slot="headline">
                  {t.name} · fgh_{t.prefix}…
                </span>
                <span slot="supporting-text">
                  {fmt(m.settings.tokenLine, {
                    scopes: t.scopes.join(' + '),
                    created: new Date(t.created_at).toLocaleDateString(),
                    usage: t.revoked_at
                      ? fmt(m.settings.tokenRevokedAt, { date: new Date(t.revoked_at).toLocaleDateString() })
                      : t.last_used_at
                        ? fmt(m.settings.tokenLastUsed, { when: relativeTime(t.last_used_at) })
                        : m.settings.tokenNeverUsed,
                    expiry: t.expires_at
                      ? fmt(m.settings.tokenExpires, { date: new Date(t.expires_at).toLocaleDateString() })
                      : m.settings.tokenNoExpiry,
                  })}
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
<div slot="headline">{m.settings.newToken}</div>
        <div slot="content" style={{ display: 'grid', gap: 'var(--fh-space-3)', minWidth: 320 }}>
          <TextField label={m.settings.tokenNameLabel} value={tokenName} onChange={setTokenName} supportingText={m.settings.tokenNameHint} />
          <Checkbox checked={readScope} onChange={setReadScope} label={m.settings.scopeRead} />
          <Checkbox
            checked={writeScope}
            onChange={setWriteScope}
            label={m.settings.scopeWrite}
          />
          <span style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center' }}>
            <SelectField
              label={m.settings.expiresLabel}
              ariaLabel={m.settings.expiryAria}
              value={expiry}
              onChange={setExpiry}
              options={[
                { value: 'never', label: m.settings.expiryNever },
                { value: '30', label: m.settings.expiry30 },
                { value: '90', label: m.settings.expiry90 },
                { value: '365', label: m.settings.expiry365 },
              ]}
            />
          </span>
        </div>
        <div slot="actions">
          <DialogActions
            confirmLabel={m.settings.createToken}
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
        headline={m.settings.revokeTokenHeadline}
        content={m.settings.revokeTokenContent}
        confirmLabel={m.settings.revokeConfirm}
        danger
        onCancel={() => setRevoking(null)}
        onConfirm={() => {
          if (revoking === null) return;
          revokeToken.mutate(revoking, {
            onSuccess: () => toast({ message: m.settings.tokenRevokedToast }),
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
        <h2 style={{ marginTop: 0 }}>{m.settings.cliTitle}</h2>
        <p>
          {m.settings.cliBody}
        </p>
        <p>{m.settings.cliSignIn}</p>
        <CodeBlock label={m.home.terminal} code={'fg login\nfg ubuntu-setup'} />
        <p style={{ margin: 0 }}>
          <code>fg login</code> {m.settings.cliGrantBefore} <code>/device</code>{m.settings.cliGrantAfter}
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
        <h2 style={{ marginTop: 0 }}>{m.settings.deleteTitle}</h2>
        <p>
          {m.settings.deleteBody}
        </p>
        <Button variant="outlined" onClick={() => setOpen(true)}>
          {m.settings.deleteMyAccount}
        </Button>
      </Card>
      <md-dialog open={open || undefined} onClose={() => setOpen(false)}>
        <div slot="headline">{m.settings.deleteHeadline}</div>
        <div slot="content" style={{ display: 'grid', gap: 'var(--fh-space-3)', minWidth: 320 }}>
          <p style={{ margin: 0 }}>
            {m.settings.deleteConfirmIntroBefore} <strong>{myName}</strong> {m.settings.deleteConfirmIntroAfter}
          </p>
          <TextField label={fmt(m.settings.typeUsername, { name: myName })} value={username} onChange={setUsername} />
          <TextField label={m.settings.passwordLabel} value={password} onChange={setPassword} type="password" autoComplete="current-password" />
          {error ? (
            <p role="alert" style={{ color: 'var(--md-sys-color-error)', margin: 0 }}>
              {error}
            </p>
          ) : null}
        </div>
        <div slot="actions">
          <DialogActions
            confirmLabel={m.settings.deleteConfirm}
            danger
            confirmDisabled={username !== myName || password === ''}
            onCancel={() => setOpen(false)}
            onConfirm={() => {
              setError(null);
              deleteAccount.mutate(
                { username, password },
                {
                  onSuccess: () => {
                    toast({ message: m.settings.deletedToast });
                    navigate('/');
                  },
                  onError: (e) =>
                    setError(
                      e instanceof ApiError && e.status === 401
                        ? m.settings.passwordMismatch
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
