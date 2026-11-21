import { useEffect, useMemo, useRef, useState } from 'react';
import { useBlocker, useNavigate, useParams } from 'react-router-dom';
import {
  Button,
  Card,
  Chip,
  ConfirmDialog,
  ErrorState,
  Icon,
  IconButton,
  PageHeader,
  SegmentedControl,
  Skeleton,
  Switch,
  Tabs,
  TextField,
  useToast,
  useWidthClass,
} from '../design';
import { ApiError } from '../api/client';
import {
  useCreateSnippet,
  useMe,
  useNameAvailability,
  usePatchSnippet,
  useSnippet,
  useUpdateSnippet,
  useValidate,
} from '../api/hooks';
import type { Snippet, ValidationResult, Visibility } from '../api/types';
import { snippetNameIssue, tagIssue } from '../validation';

const MAX_BODY_BYTES = 65536;
const VISIBILITY_HELP: { id: Visibility; label: string; help: string }[] = [
  { id: 'private', label: 'Private', help: 'Only you can see and run it.' },
  { id: 'unlisted', label: 'Unlisted', help: 'Anyone with the link can see it; it is not listed or searchable.' },
  { id: 'public', label: 'Public', help: 'Anyone can find, read and run it. Review it for secrets first.' },
];

type Mode = 'script' | 'steps';

type InitialValues = {
  name: string;
  description: string;
  visibility: Visibility;
  tags: string[];
  body: string;
  mode: Mode;
};

/** Route wrapper: in edit mode, fetch the snippet first; the
 * form itself is a separate component so its state can be
 * initialised from props (no set-state-in-effect). */
export default function EditorPage() {
  const params = useParams<{ owner: string; name: string }>();
  const editing = params.owner !== undefined && params.name !== undefined;
  const existing = useSnippet(params.owner ?? '', params.name ?? '');
  const navigate = useNavigate();

  if (!editing) {
    return (
      <EditorForm
        initial={{ name: '', description: '', visibility: 'private', tags: [], body: '', mode: 'script' }}
      />
    );
  }
  if (existing.isPending) {
    return (
      <Card variant="outlined">
        <Skeleton lines={5} />
      </Card>
    );
  }
  if (existing.isError || !existing.data) {
    return (
      <ErrorState
        title="Snippet not found"
        message="This snippet does not exist, or it is private."
        retryLabel="Back to dashboard"
        onRetry={() => navigate('/dashboard')}
      />
    );
  }
  const s = existing.data;
  return (
    <EditorForm
      existing={s}
      initial={{
        name: s.name,
        description: s.description,
        visibility: s.visibility,
        tags: s.tags,
        body: s.body,
        mode: s.summary?.mode ?? 'script',
      }}
    />
  );
}

function EditorForm({ initial, existing }: { initial: InitialValues; existing?: Snippet }) {
  const editing = existing !== undefined;
  const me = useMe();
  const navigate = useNavigate();
  const toast = useToast();
  const widthClass = useWidthClass();

  const [name, setName] = useState(initial.name);
  const [description, setDescription] = useState(initial.description);
  const [visibility, setVisibility] = useState<Visibility>(initial.visibility);
  const [tags, setTags] = useState<string[]>(initial.tags);
  const [tagDraft, setTagDraft] = useState('');
  const [mode, setMode] = useState<Mode>(initial.mode);
  const [body, setBody] = useState(initial.body);
  const [message, setMessage] = useState('');
  const [wrap, setWrap] = useState(true);
  const [showLines, setShowLines] = useState(false);
  const [validation, setValidation] = useState<ValidationResult | null>(null);
  const [validating, setValidating] = useState(false);
  const [conflict, setConflict] = useState(false);
  const escArmed = useRef(false);
  const gutterRef = useRef<HTMLDivElement>(null);

  const [base, setBase] = useState(initial);
  const dirty = useMemo(() => {
    if (!editing) return name !== '' || body !== '' || description !== '' || tags.length > 0;
    return (
      description !== base.description ||
      visibility !== base.visibility ||
      body !== base.body ||
      JSON.stringify(tags) !== JSON.stringify(base.tags)
    );
  }, [editing, name, body, description, tags, visibility, base]);

  // After a successful save we navigate away ourselves; the
  // guard must stand down for that one navigation or it would
  // block the post-save redirect as if it were a discard. The
  // blocker is a function so it reads this ref at navigation
  // time (a state flag would race the blocker's own update).
  const allowLeave = useRef(false);
  const navigateAfterSave = (path: string) => {
    allowLeave.current = true;
    navigate(path, { replace: true });
  };

  // Unsaved-changes guard: router navigation + tab close.
  const blocker = useBlocker(() => dirty && !allowLeave.current);
  useEffect(() => {
    if (!dirty) return;
    const onBeforeUnload = (e: BeforeUnloadEvent) => e.preventDefault();
    window.addEventListener('beforeunload', onBeforeUnload);
    return () => window.removeEventListener('beforeunload', onBeforeUnload);
  }, [dirty]);

  // Debounced server validation (the live panel).
  const validate = useValidate();
  const validateMutate = validate.mutate;
  useEffect(() => {
    const timer = window.setTimeout(() => {
      setValidating(true);
      validateMutate(
        { name: name || 'untitled-snippet', body, mode },
        {
          onSuccess: (result) => {
            setValidation(result);
            setValidating(false);
          },
          onError: () => setValidating(false),
        },
      );
    }, 500);
    return () => window.clearTimeout(timer);
  }, [name, body, mode, validateMutate]);

  // Debounced name availability (new snippets only).
  const [debouncedName, setDebouncedName] = useState('');
  useEffect(() => {
    const timer = window.setTimeout(() => setDebouncedName(name), 400);
    return () => window.clearTimeout(timer);
  }, [name]);
  const availability = useNameAvailability(me.data?.username, debouncedName, !editing);

  const bodyBytes = useMemo(() => new TextEncoder().encode(body).length, [body]);
  const overSize = bodyBytes > MAX_BODY_BYTES;
  const nameIssue = snippetNameIssue(name);
  const secrets = validation?.secret_findings ?? [];
  const saveBlocked =
    name.length === 0 ||
    !!nameIssue ||
    (!editing && availability.data === 'taken') ||
    overSize ||
    secrets.length > 0 ||
    validation?.valid === false ||
    body.trim() === '';

  const create = useCreateSnippet();
  const update = useUpdateSnippet();
  const patch = usePatchSnippet();

  const onSaveError = (error: ApiError) => {
    if (error.status === 412) {
      setConflict(true);
    } else if (error.status === 422) {
      toast({ message: 'The server found secrets in this snippet. Remove them before saving.' });
    } else if (error.code === 'name_taken') {
      toast({ message: 'You already have a snippet with that name.' });
    } else {
      toast({ message: error.uiMessage });
    }
  };

  const save = () => {
    const input = {
      name,
      description: description || undefined,
      visibility,
      tags,
      mode,
      body,
      message: message || undefined,
    };
    if (!editing) {
      create.mutate(input, {
        onSuccess: (snippet) => {
          toast({ message: `${snippet.name} saved` });
          navigateAfterSave(`/${snippet.owner}/${snippet.name}`);
        },
        onError: onSaveError,
      });
      return;
    }
    const s = existing;
    const metaChanged =
      description !== s.description || visibility !== s.visibility || JSON.stringify(tags) !== JSON.stringify(s.tags);
    const bodyChanged = body !== s.body;
    const afterMeta = () => {
      if (!bodyChanged) {
        toast({ message: 'Saved' });
        navigateAfterSave(`/${s.owner}/${s.name}`);
        return;
      }
      update.mutate(
        { owner: s.owner, name: s.name, ifMatch: s.version, input },
        {
          onSuccess: (saved) => {
            toast({ message: `Saved as version ${saved.version}` });
            navigateAfterSave(`/${saved.owner}/${saved.name}`);
          },
          onError: onSaveError,
        },
      );
    };
    if (metaChanged) {
      patch.mutate(
        { owner: s.owner, name: s.name, patch: { description, visibility, tags } },
        { onSuccess: afterMeta, onError: onSaveError },
      );
    } else {
      afterMeta();
    }
  };

  const overwriteConflict = () => {
    setConflict(false);
    if (!existing) return;
    // Re-fetch to learn the latest version, then retry the
    // PUT against it with the user's text.
    void fetchLatest(existing).then((fresh) => {
      if (!fresh) return;
      update.mutate(
        {
          owner: fresh.owner,
          name: fresh.name,
          ifMatch: fresh.version,
          input: { name, description, visibility, tags, mode, body, message: message || undefined },
        },
        {
          onSuccess: (saved) => {
            toast({ message: `Saved as version ${saved.version}` });
            navigateAfterSave(`/${saved.owner}/${saved.name}`);
          },
          onError: onSaveError,
        },
      );
    });
  };

  /** Conflict resolution: take the server's latest version
   * into the form (fields and dirty-base both reset). */
  const reloadTheirs = () => {
    if (!existing) return;
    void fetchLatest(existing).then((fresh) => {
      if (!fresh) {
        toast({ message: 'Could not load the latest version - check your connection' });
        return;
      }
      const freshInitial = {
        name: fresh.name,
        description: fresh.description,
        visibility: fresh.visibility,
        tags: fresh.tags,
        body: fresh.body,
        mode: fresh.summary?.mode ?? ('script' as Mode),
      };
      setBase(freshInitial);
      setDescription(fresh.description);
      setVisibility(fresh.visibility);
      setTags(fresh.tags);
      setBody(fresh.body);
      setMode(freshInitial.mode);
      setMessage('');
      setConflict(false);
      toast({ message: `Loaded version ${fresh.version}` });
    });
  };

  // Ctrl+S / Cmd+S saves from anywhere in the form.
  const saveRef = useRef<() => void>(() => {});
  saveRef.current = save;
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
        e.preventDefault();
        saveRef.current();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const addTag = () => {
    const t = tagDraft.trim().toLowerCase();
    if (!t) return;
    const issue = tagIssue(t);
    if (issue) {
      toast({ message: issue });
      return;
    }
    if (tags.includes(t)) {
      setTagDraft('');
      return;
    }
    if (tags.length >= 20) {
      toast({ message: 'At most 20 tags.' });
      return;
    }
    setTags([...tags, t]);
    setTagDraft('');
  };

  const onBodyKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Escape') {
      escArmed.current = true;
      return;
    }
    if (e.key === 'Tab') {
      if (escArmed.current) {
        // Esc, then Tab leaves the editor (no keyboard trap).
        escArmed.current = false;
        return;
      }
      e.preventDefault();
      const el = e.currentTarget;
      const start = el.selectionStart;
      el.setRangeText('  ', start, el.selectionEnd, 'end');
      setBody(el.value);
    } else {
      escArmed.current = false;
    }
  };

  const panel = (
    <Card variant="outlined">
      <h2 style={{ marginTop: 0 }}>Checks</h2>
      {validating && !validation ? <p>Checking…</p> : null}
      {secrets.length > 0 ? (
        <div role="alert">
          <p style={{ color: 'var(--md-sys-color-error)', fontWeight: 500 }}>
            Secrets found - saving is blocked. The values are never shown or sent anywhere else.
          </p>
          <ul>
            {secrets.map((f, i) => (
              <li key={i}>
                Line {f.line}: possible {f.kind.replace(/_/g, ' ')}
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      {validation && !validation.valid && secrets.length === 0 ? (
        <div role="alert">
          <p style={{ fontWeight: 500 }}>This snippet has problems:</p>
          <ul>
            {validation.errors.map((err, i) => (
              <li key={i}>
                {err.line ? `Line ${err.line}: ` : ''}
                {err.message}
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      {validation?.valid ? <p>✓ Valid - ready to save.</p> : null}
      {validation?.summary ? (
        <ul style={{ paddingLeft: 'var(--fh-space-4)' }}>
          <li>
            {validation.summary.mode === 'steps'
              ? `${validation.summary.steps_count} steps`
              : `${body.split('\n').length} lines`}
          </li>
          <li>
            Size: {bodyBytes.toLocaleString()} / {MAX_BODY_BYTES.toLocaleString()} bytes
            {overSize ? ' - too large' : ''}
          </li>
          {validation.summary.uses_sudo ? <li>Uses sudo</li> : null}
          {validation.summary.variables.length > 0 ? (
            <li>
              Variables:{' '}
              {validation.summary.variables.map((v) => (
                <Chip key={v} label={v} />
              ))}
            </li>
          ) : null}
        </ul>
      ) : null}
      {validation?.summary && validation.summary.lint_warnings.length > 0 ? (
        <div>
          <p style={{ fontWeight: 500 }}>Warnings (not blocking):</p>
          <ul>
            {validation.summary.lint_warnings.map((w, i) => (
              <li key={i}>
                <Icon name="warning" /> Line {w.line}: {w.message}
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      <p style={{ color: 'var(--md-sys-color-on-surface-variant)' }}>
        Tab inserts spaces. Press Esc, then Tab, to leave the editor with the keyboard.
      </p>
    </Card>
  );

  const form = (
    <div style={{ display: 'grid', gap: 'var(--fh-space-4)' }}>
      <TextField
        label="Name"
        value={name}
        onChange={setName}
        disabled={editing}
        errorText={
          nameIssue ??
          (!editing && availability.data === 'taken' ? 'You already have a snippet with this name.' : undefined)
        }
        supportingText={
          editing
            ? 'Names cannot change here; rename from the CLI for now.'
            : !editing && availability.data === 'free'
              ? 'Available.'
              : 'The name for fg commands, e.g. ubuntu-setup.'
        }
      />
      <TextField label="Description" value={description} onChange={setDescription} maxLength={280} supportingText="At most 280 characters." />
      <div>
        <p style={{ fontWeight: 500, margin: '0 0 var(--fh-space-2)' }}>Visibility</p>
        <SegmentedControl
          ariaLabel="Visibility"
          options={VISIBILITY_HELP.map((v) => ({ id: v.id, label: v.label }))}
          value={visibility}
          onChange={setVisibility}
        />
        <p style={{ color: 'var(--md-sys-color-on-surface-variant)', margin: 'var(--fh-space-2) 0 0' }}>
          {VISIBILITY_HELP.find((v) => v.id === visibility)?.help}
        </p>
      </div>
      <div>
        <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'flex-start' }}>
          <div style={{ flex: 1 }}>
            <TextField label="Add a tag" value={tagDraft} onChange={setTagDraft} supportingText={`${tags.length} / 20 tags`} />
          </div>
          <Button variant="tonal" onClick={addTag}>
            Add
          </Button>
        </div>
        {tags.length > 0 ? (
          <p style={{ display: 'flex', gap: 'var(--fh-space-1)', flexWrap: 'wrap' }}>
            {tags.map((tag) => (
              <span key={tag} style={{ display: 'inline-flex', alignItems: 'center', gap: 'var(--fh-space-1)' }}>
                <Chip label={tag} />
                <IconButton icon="close" label={`Remove tag ${tag}`} onClick={() => setTags(tags.filter((t) => t !== tag))} />
              </span>
            ))}
          </p>
        ) : null}
      </div>
      <Tabs
        ariaLabel="Body mode"
        tabs={[
          { id: 'script', label: 'Script' },
          { id: 'steps', label: 'Steps (YAML)' },
        ]}
        activeIndex={mode === 'script' ? 0 : 1}
        onChange={(i) => setMode(i === 0 ? 'script' : 'steps')}
      />
      <div style={{ display: 'flex', gap: 'var(--fh-space-3)', alignItems: 'center', flexWrap: 'wrap' }}>
        <Switch checked={wrap} onChange={setWrap} label="Soft wrap" />
        <Switch checked={showLines} onChange={setShowLines} label="Line numbers" />
      </div>
      <div style={{ display: 'flex', background: 'var(--fh-code-bg)', borderRadius: 'var(--md-sys-shape-corner-medium)', overflow: 'hidden' }}>
        {showLines ? (
          <div
            ref={gutterRef}
            aria-hidden="true"
            style={{
              padding: 'var(--fh-space-3) var(--fh-space-2)',
              textAlign: 'right',
              color: 'var(--md-sys-color-on-surface-variant)',
              fontFamily: "'Roboto Mono', monospace",
              fontSize: 'var(--md-sys-typescale-body-medium-size)',
              lineHeight: 'var(--md-sys-typescale-body-medium-line-height)',
              overflow: 'hidden',
              userSelect: 'none',
            }}
          >
            {body.split('\n').map((_, i) => (
              <div key={i}>{i + 1}</div>
            ))}
          </div>
        ) : null}
        <textarea
          aria-label={mode === 'script' ? 'Script body' : 'YAML body'}
          value={body}
          onChange={(e) => setBody(e.target.value)}
          onKeyDown={onBodyKeyDown}
          onScroll={(e) => {
            if (gutterRef.current) gutterRef.current.scrollTop = e.currentTarget.scrollTop;
          }}
          placeholder={mode === 'script' ? 'sudo apt-get update\nsudo apt-get install -y curl git' : 'name: my-snippet\nsteps:\n  - run: echo hello'}
          rows={14}
          wrap={wrap ? 'soft' : 'off'}
          style={{
            flex: 1,
            border: 'none',
            background: 'none',
            color: 'var(--md-sys-color-on-surface)',
            fontFamily: "'Roboto Mono', monospace",
            fontSize: 'var(--md-sys-typescale-body-medium-size)',
            lineHeight: 'var(--md-sys-typescale-body-medium-line-height)',
            padding: 'var(--fh-space-3)',
            resize: 'vertical',
            whiteSpace: wrap ? 'pre-wrap' : 'pre',
          }}
        />
      </div>
      {editing ? (
        <TextField label="Version message (optional)" value={message} onChange={setMessage} maxLength={200} />
      ) : null}
      <p style={{ display: 'flex', gap: 'var(--fh-space-2)', margin: 0 }}>
        <Button variant="filled" onClick={save} disabled={saveBlocked || create.isPending || update.isPending || patch.isPending}>
          {editing ? 'Save new version' : 'Save snippet'}
        </Button>
        <Button variant="text" onClick={() => navigate(-1)}>
          Cancel
        </Button>
      </p>
    </div>
  );

  return (
    <div>
      <PageHeader
        title={editing ? `Edit ${initial.name}` : 'New snippet'}
        subtitle={editing ? 'Saving creates a new version.' : 'Private by default. Share only when you are ready.'}
      />
      {widthClass === 'expanded' ? (
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 340px', gap: 'var(--fh-space-4)', alignItems: 'start' }}>
          <div>{form}</div>
          <div style={{ position: 'sticky', top: 'var(--fh-space-4)' }}>{panel}</div>
        </div>
      ) : (
        <div style={{ display: 'grid', gap: 'var(--fh-space-4)' }}>
          {form}
          {panel}
        </div>
      )}

      <ConfirmDialog
        open={blocker.state === 'blocked'}
        headline="Discard unsaved changes?"
        content="You have edits that are not saved yet."
        confirmLabel="Discard"
        cancelLabel="Keep editing"
        danger
        onConfirm={() => blocker.proceed?.()}
        onCancel={() => blocker.reset?.()}
      />
      <md-dialog open={conflict || undefined} onClose={() => setConflict(false)}>
        <div slot="headline">Someone saved a newer version</div>
        <div slot="content">
          This snippet changed since you opened it - probably from the CLI. Reload their version
          into this form, keep editing yours, or save yours as the next version on top of theirs.
          Nothing is overwritten silently.
        </div>
        <div slot="actions" style={{ display: 'flex', gap: 'var(--fh-space-2)', flexWrap: 'wrap' }}>
          <Button variant="text" onClick={reloadTheirs}>
            Reload theirs
          </Button>
          <Button variant="text" onClick={() => setConflict(false)}>
            Keep editing
          </Button>
          <Button variant="filled" onClick={overwriteConflict}>
            Save mine as new version
          </Button>
        </div>
      </md-dialog>
    </div>
  );
}

/** Plain fetch of the latest snippet for the conflict retry
 * (goes through the same client so CSRF/auth rules apply). */
async function fetchLatest(existing: Snippet): Promise<Snippet | null> {
  const { api } = await import('../api/client');
  try {
    return await api<Snippet>(
      `/snippets/${encodeURIComponent(existing.owner)}/${encodeURIComponent(existing.name)}`,
    );
  } catch {
    return null;
  }
}
