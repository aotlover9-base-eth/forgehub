import { useRef, useState } from 'react';
import { Button, Card, Checkbox, TextField, useToast } from '../design';
import { useCreateSnippet, useGistImport, useHistoryAnalyze } from '../api/hooks';
import type { HistoryCandidate } from '../api/types';

const MAX_HISTORY_BYTES = 64 * 1024;

/** The forge-file body for one mined history candidate. */
function stepsBody(name: string, commands: string[]): string {
  const lines = commands.map((c) => `  - run: ${JSON.stringify(c)}`);
  return `name: ${name}\ndescription: Imported from shell history\nsteps:\n${lines.join('\n')}\n`;
}

interface PickedCandidate extends HistoryCandidate {
  picked: boolean;
  state: 'idle' | 'done' | 'error';
  error?: string;
}

/**
 * The Library's Import section (Peak C5): bring snippets in
 * from a public GitHub Gist (the server fetches it, ADR
 * 0010) or from a shell history file (analyzed server-side
 * by the same mining pipeline as `fg analyze` - secrets are
 * filtered out before candidates ever come back).
 */
export default function ImportPanel() {
  const toast = useToast();
  const gistImport = useGistImport();
  const analyze = useHistoryAnalyze();
  const createSnippet = useCreateSnippet();
  const [gistUrl, setGistUrl] = useState('');
  const [candidates, setCandidates] = useState<PickedCandidate[] | null>(null);
  const [analysisNote, setAnalysisNote] = useState('');
  const [importing, setImporting] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  const submitGist = () => {
    const url = gistUrl.trim();
    if (!url) {
      toast({ message: 'Paste a gist URL first' });
      return;
    }
    gistImport.mutate(
      { url },
      {
        onSuccess: (result) => {
          if (result.created.length > 0) {
            toast({
              message: `Imported ${result.created.map((c) => c.name).join(', ')} (private)`,
            });
          } else {
            toast({ message: 'Nothing was imported - see the skipped files below' });
          }
        },
        onError: (error) => toast({ message: error.uiMessage }),
      },
    );
  };

  const onFile = async (file: File | undefined) => {
    if (!file) return;
    if (file.size > MAX_HISTORY_BYTES) {
      toast({ message: 'That history file is over the 64 KiB limit' });
      return;
    }
    const text = await file.text();
    analyze.mutate(
      { text },
      {
        onSuccess: (result) => {
          setCandidates(
            result.candidates.map((c) => ({ ...c, picked: true, state: 'idle' as const })),
          );
          setAnalysisNote(
            `${result.parsed} commands read, ${result.kept} kept after noise and secret filtering, ${result.candidates.length} repeated workflow${result.candidates.length === 1 ? '' : 's'} found.`,
          );
        },
        onError: (error) => toast({ message: error.uiMessage }),
      },
    );
  };

  const importPicked = async () => {
    if (!candidates) return;
    setImporting(true);
    const next = [...candidates];
    for (let i = 0; i < next.length; i++) {
      const c = next[i];
      if (!c.picked || c.state === 'done') continue;
      try {
        await createSnippet.mutateAsync({
          name: c.name,
          body: stepsBody(c.name, c.commands),
        });
        next[i] = { ...c, state: 'done' };
      } catch (error) {
        next[i] = {
          ...c,
          state: 'error',
          error: error instanceof Error ? error.message : 'failed',
        };
      }
      setCandidates([...next]);
    }
    setImporting(false);
    const done = next.filter((c) => c.state === 'done').length;
    toast({ message: `Imported ${done} workflow${done === 1 ? '' : 's'} from your history` });
  };

  return (
    <Card>
      <div style={{ display: 'grid', gap: 'var(--fh-space-4)', padding: 'var(--fh-space-4)' }}>
        <h2 style={{ margin: 0 }}>Import</h2>

        <div style={{ display: 'grid', gap: 'var(--fh-space-2)' }}>
          <h3 style={{ margin: 0 }}>From a GitHub Gist</h3>
          <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'flex-end', flexWrap: 'wrap' }}>
            <div style={{ flex: 1, minWidth: 260 }}>
              <TextField
                label="Gist URL"
                value={gistUrl}
                onChange={setGistUrl}
                supportingText="Public gists only. Files become private snippets; anything that looks like a secret is skipped."
              />
            </div>
            <Button variant="tonal" icon="download" onClick={submitGist} disabled={gistImport.isPending}>
              Import gist
            </Button>
          </div>
          {gistImport.data ? (
            <div>
              {gistImport.data.created.map((c) => (
                <p key={c.name} style={{ margin: 0 }}>
                  Imported <code>{c.name}</code> (from {c.file})
                </p>
              ))}
              {gistImport.data.skipped.map((s) => (
                <p key={s.file} style={{ margin: 0 }}>
                  Skipped {s.file}: {s.reason}
                </p>
              ))}
              {gistImport.data.created.length === 0 && gistImport.data.skipped.length === 0 ? (
                <p style={{ margin: 0 }}>The gist has no files to import.</p>
              ) : null}
            </div>
          ) : null}
        </div>

        <div style={{ display: 'grid', gap: 'var(--fh-space-2)' }}>
          <h3 style={{ margin: 0 }}>From your shell history</h3>
          <p style={{ margin: 0 }}>
            Upload a history file (bash, zsh or fish, up to 64 KiB). It is analyzed on the
            server with the same pipeline as <code>fg analyze</code> - commands that look
            like secrets are filtered out before anything comes back.
          </p>
          <div>
            <input
              ref={fileRef}
              type="file"
              aria-label="Shell history file"
              style={{ display: 'none' }}
              onChange={(e) => {
                void onFile(e.target.files?.[0]);
                e.target.value = '';
              }}
            />
            <Button
              variant="tonal"
              icon="upload"
              onClick={() => fileRef.current?.click()}
              disabled={analyze.isPending}
            >
              Choose history file
            </Button>
          </div>
          {analysisNote ? <p style={{ margin: 0 }}>{analysisNote}</p> : null}
          {candidates && candidates.length === 0 ? (
            <p style={{ margin: 0 }}>No repeated workflows found in that file.</p>
          ) : null}
          {candidates && candidates.length > 0 ? (
            <>
              <div style={{ display: 'grid', gap: 'var(--fh-space-2)' }}>
                {candidates.map((c, i) => (
                  <div key={`${c.name}-${i}`} style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'flex-start' }}>
                    <Checkbox
                      ariaLabel={`Import ${c.name}`}
                      checked={c.picked}
                      disabled={c.state === 'done'}
                      onChange={(picked) =>
                        setCandidates(
                          candidates.map((x, xi) => (xi === i ? { ...x, picked } : x)),
                        )
                      }
                    />
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'flex-end' }}>
                        <div style={{ minWidth: 180 }}>
                          <TextField
                            label="Snippet name"
                            value={c.name}
                            onChange={(name) =>
                              setCandidates(
                                candidates.map((x, xi) => (xi === i ? { ...x, name } : x)),
                              )
                            }
                          />
                        </div>
                        <span>
                          seen {c.count} times
                          {c.state === 'done' ? ' - imported' : ''}
                          {c.state === 'error' ? ` - failed: ${c.error ?? ''}` : ''}
                        </span>
                      </div>
                      <code style={{ display: 'block', whiteSpace: 'pre-wrap' }}>
                        {c.commands.join('\n')}
                      </code>
                    </div>
                  </div>
                ))}
              </div>
              <div>
                <Button variant="filled" icon="download" onClick={() => void importPicked()} disabled={importing}>
                  Import selected
                </Button>
              </div>
            </>
          ) : null}
        </div>
      </div>
    </Card>
  );
}
