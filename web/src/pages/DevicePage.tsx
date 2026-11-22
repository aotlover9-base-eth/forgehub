import { useState } from 'react';
import { Navigate, useSearchParams } from 'react-router-dom';
import { Button, Card, ErrorState, Icon, PageHeader, Skeleton, TextField } from '../design';
import { ApiError } from '../api/client';
import { useDeviceDecision, useDeviceLookup, useMe } from '../api/hooks';
import { m } from '../i18n/messages';

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
      <PageHeader title={m.device.title} subtitle={m.device.subtitle} />
      {outcome === 'approved' ? (
        <Card variant="filled">
          <p style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center', margin: 0 }}>
            <Icon name="check" /> {m.device.approved}
          </p>
        </Card>
      ) : outcome === 'denied' ? (
        <Card variant="filled">
          <p style={{ margin: 0 }}>
            {m.device.denied}
          </p>
        </Card>
      ) : !submittedCode ? (
        <Card variant="outlined">
          <p>{m.device.enterCode}</p>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              setSubmittedCode(code.trim().toUpperCase());
            }}
            style={{ display: 'grid', gap: 'var(--fh-space-3)' }}
          >
            <TextField label={m.device.codeLabel} value={code} onChange={(v) => setCode(v.toUpperCase())} required />
            <Button variant="filled" type="submit" disabled={code.trim().length < 8}>
              {m.device.continue}
            </Button>
          </form>
        </Card>
      ) : lookup.isPending || me.isPending ? (
        <Card variant="outlined">
          <Skeleton lines={3} />
        </Card>
      ) : lookup.isError || !lookup.data ? (
        <ErrorState
          title={m.device.badCodeTitle}
          message={
            lookup.error instanceof ApiError && lookup.error.status === 429
              ? lookup.error.uiMessage
              : m.device.badCodeMessage
          }
          retryLabel={m.device.differentCode}
          onRetry={() => {
            setSubmittedCode(null);
            setCode('');
          }}
        />
      ) : (
        <Card variant="outlined">
          <p style={{ marginTop: 0 }}>
            <strong>{lookup.data.client_name}</strong> {m.device.askingAfter}
          </p>
          <p>
            {m.device.accessBefore} <strong>{m.device.accessStrong}</strong> {m.device.accessMiddle} <code>fg push</code> {m.device.accessAnd} <code>fg pull</code>{m.device.accessAfter}
          </p>
          <p style={{ color: 'var(--md-sys-color-on-surface-variant)' }}>
            {m.device.codeWord} <code>{submittedCode}</code> · {m.device.expiresWord}{' '}
            {new Date(lookup.data.expires_at).toLocaleTimeString()}
          </p>
          {decision.isError ? (
            <p role="alert" style={{ color: 'var(--md-sys-color-error)' }}>
              {decision.error instanceof ApiError ? decision.error.uiMessage : m.common.somethingWrongShort}
            </p>
          ) : null}
          <p style={{ display: 'flex', gap: 'var(--fh-space-2)', margin: 0 }}>
            <Button variant="filled" onClick={() => decide(true)} disabled={decision.isPending}>
              {m.device.approve}
            </Button>
            <Button variant="outlined" onClick={() => decide(false)} disabled={decision.isPending}>
              {m.device.deny}
            </Button>
          </p>
        </Card>
      )}
      <p style={{ color: 'var(--md-sys-color-on-surface-variant)', display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center' }}>
        <Icon name="shield" /> {m.device.onlyYours}
      </p>
    </div>
  );
}
