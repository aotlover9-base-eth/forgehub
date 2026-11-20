import { useState } from 'react';
import { Navigate, useSearchParams } from 'react-router-dom';
import { Button, Card, ErrorState, Icon, PageHeader, Skeleton, TextField } from '../design';
import { ApiError } from '../api/client';
import { useDeviceDecision, useDeviceLookup, useMe } from '../api/hooks';

/**
 * The CLI login approval page - security-critical. It never
 * auto-approves: a human reads the client name and chooses.
 * The code arrives prefilled from verification_uri_complete
 * (?code=) or is typed by hand. Brute-force protection is
 * server-side (per-IP lockout); its 429 surfaces here as a
 * plain wait message.
 */
export default function DevicePage() {
  const [params] = useSearchParams();
  const me = useMe();
  const [code, setCode] = useState(() => (params.get('code') ?? '').toUpperCase());
  const [submittedCode, setSubmittedCode] = useState<string | null>(() => {
    const fromUrl = (params.get('code') ?? '').toUpperCase();
    return fromUrl ? fromUrl : null;
  });
  const [outcome, setOutcome] = useState<'approved' | 'denied' | null>(null);
  const lookup = useDeviceLookup(submittedCode);
  const decision = useDeviceDecision();

  // Not signed in: go to login and come back here (the code
  // rides along in the return URL).
  if (!me.isPending && !me.data) {
    const ret = `/device${submittedCode ? `?code=${encodeURIComponent(submittedCode)}` : ''}`;
    return <Navigate to={`/login?return=${encodeURIComponent(ret)}`} replace />;
  }

  const decide = (approve: boolean) => {
    if (!submittedCode) return;
    decision.mutate(
      { user_code: submittedCode, approve },
      {
        onSuccess: () => setOutcome(approve ? 'approved' : 'denied'),
        onError: () => {
          // The code may have expired between lookup and
          // decision; the lookup refetch shows the state.
          void lookup.refetch();
        },
      },
    );
  };

  return (
    <div style={{ maxWidth: 520, margin: '0 auto', display: 'grid', gap: 'var(--fh-space-4)' }}>
      <PageHeader title="Sign in a CLI" subtitle="A ForgeHub CLI is asking to sign in as you." />
      {outcome === 'approved' ? (
        <Card variant="filled">
          <p style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center', margin: 0 }}>
            <Icon name="check" /> Approved. You can close this tab - the CLI is signed in now.
          </p>
        </Card>
      ) : outcome === 'denied' ? (
        <Card variant="filled">
          <p style={{ margin: 0 }}>
            Denied. The CLI will not be signed in. If this was not you, your account is still safe -
            nothing was granted.
          </p>
        </Card>
      ) : !submittedCode ? (
        <Card variant="outlined">
          <p>Enter the code the CLI is showing you:</p>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              setSubmittedCode(code.trim().toUpperCase());
            }}
            style={{ display: 'grid', gap: 'var(--fh-space-3)' }}
          >
            <TextField label="Device code" value={code} onChange={(v) => setCode(v.toUpperCase())} required />
            <Button variant="filled" type="submit" disabled={code.trim().length < 8}>
              Continue
            </Button>
          </form>
        </Card>
      ) : lookup.isPending || me.isPending ? (
        <Card variant="outlined">
          <Skeleton lines={3} />
        </Card>
      ) : lookup.isError || !lookup.data ? (
        <ErrorState
          title="That code does not work"
          message={
            lookup.error instanceof ApiError && lookup.error.status === 429
              ? lookup.error.uiMessage
              : 'The code is wrong, expired, or already used. Check the CLI and try the code it shows now.'
          }
          retryLabel="Enter a different code"
          onRetry={() => {
            setSubmittedCode(null);
            setCode('');
          }}
        />
      ) : (
        <Card variant="outlined">
          <p style={{ marginTop: 0 }}>
            <strong>{lookup.data.client_name}</strong> is asking to sign in.
          </p>
          <p>
            Approving creates a CLI token that can <strong>read and write your snippets</strong> -
            the same access as <code>fg push</code> and <code>fg pull</code>. You can revoke it any
            time in Settings → Tokens.
          </p>
          <p style={{ color: 'var(--md-sys-color-on-surface-variant)' }}>
            Code <code>{submittedCode}</code> · expires{' '}
            {new Date(lookup.data.expires_at).toLocaleTimeString()}
          </p>
          {decision.isError ? (
            <p role="alert" style={{ color: 'var(--md-sys-color-error)' }}>
              {decision.error instanceof ApiError ? decision.error.uiMessage : 'Something went wrong.'}
            </p>
          ) : null}
          <p style={{ display: 'flex', gap: 'var(--fh-space-2)', margin: 0 }}>
            <Button variant="filled" onClick={() => decide(true)} disabled={decision.isPending}>
              Approve
            </Button>
            <Button variant="outlined" onClick={() => decide(false)} disabled={decision.isPending}>
              Deny
            </Button>
          </p>
        </Card>
      )}
      <p style={{ color: 'var(--md-sys-color-on-surface-variant)', display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center' }}>
        <Icon name="shield" /> Only approve a code you just requested from your own terminal.
      </p>
    </div>
  );
}
