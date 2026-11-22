import { useRef, useState } from 'react';
import { Button, Card, Checkbox, TextField, useToast } from '../design';
import { useCreateSnippet, useGistImport, useHistoryAnalyze } from '../api/hooks';
import type { HistoryCandidate } from '../api/types';
import { fmt, m } from '../i18n/messages';

const MAX_HISTORY_BYTES = 64 * 1024;

/** The forge-file body for one mined history candidate. */
function stepsBody(name: string, commands: string[]): string {
  const lines = commands.map((c) => `  - run: ${JSON.stringify(c)}`);
  return `name: ${name}\ndescription: ${m.importPanel.historyDescription}\nsteps:\n${lines.join('\n')}\n`;
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
      toast({ message: m.importPanel.gistPasteFirst });
      return;
    }
    gistImport.mutate(
      { url },
      {
        onSuccess: (result) => {
          if (result.created.length > 0) {
            toast({
              message: fmt(m.importPanel.gistImported, { names: result.created.map((c) => c.name).join(', ') }),
            });
          } else {
            toast({ message: m.importPanel.gistNothing });
          }
        },
        onError: (error) => toast({ message: error.uiMessage }),
      },
    );
  };

  const onFile = async (file: File | undefined) => {
    if (!file) return;
    if (file.size > MAX_HISTORY_BYTES) {
      toast({ message: m.importPanel.historyTooBig });
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
            fmt(m.importPanel.historyAnalysis, { parsed: result.parsed, kept: result.kept, found: result.candidates.length, workflow: result.candidates.length === 1 ? m.importPanel.workflowOne : m.importPanel.workflowMany }),
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
    toast({ message: fmt(m.importPanel.historyImportedToast, { count: done, workflow: done === 1 ? m.importPanel.workflowOne : m.importPanel.workflowMany }) });
  };

  return (
    <Card>
      <div style={{ display: 'grid', gap: 'var(--fh-space-4)', padding: 'var(--fh-space-4)' }}>
        <h2 style={{ margin: 0 }}>{m.importPanel.title}</h2>

        <div style={{ display: 'grid', gap: 'var(--fh-space-2)' }}>
          <h3 style={{ margin: 0 }}>{m.importPanel.gistTitle}</h3>
          <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'flex-end', flexWrap: 'wrap' }}>
            <div style={{ flex: 1, minWidth: 260 }}>
              <TextField
                label={m.importPanel.gistUrl}
                value={gistUrl}
                onChange={setGistUrl}
                supportingText={m.importPanel.gistHint}
              />
            </div>
            <Button variant="tonal" icon="download" onClick={submitGist} disabled={gistImport.isPending}>
              {m.importPanel.gistSubmit}
            </Button>
          </div>
          {gistImport.data ? (
            <div>
              {gistImport.data.created.map((c) => (
                <p key={c.name} style={{ margin: 0 }}>
                  {m.importPanel.gistImportedLine} <code>{c.name}</code> ({m.importPanel.gistFrom} {c.file})
                </p>
              ))}
              {gistImport.data.skipped.map((s) => (
                <p key={s.file} style={{ margin: 0 }}>
                  {fmt(m.importPanel.gistSkippedLine, { file: s.file, reason: s.reason })}
                </p>
              ))}
              {gistImport.data.created.length === 0 && gistImport.data.skipped.length === 0 ? (
                <p style={{ margin: 0 }}>{m.importPanel.gistEmpty}</p>
              ) : null}
            </div>
          ) : null}
        </div>

        <div style={{ display: 'grid', gap: 'var(--fh-space-2)' }}>
          <h3 style={{ margin: 0 }}>{m.importPanel.historyTitle}</h3>
          <p style={{ margin: 0 }}>
            {m.importPanel.historyIntroBefore} <code>fg analyze</code> {m.importPanel.historyIntroAfter}
          </p>
          <div>
            <input
              ref={fileRef}
              type="file"
              aria-label={m.importPanel.historyFileLabel}
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
              {m.importPanel.historyChoose}
            </Button>
          </div>
          {analysisNote ? <p style={{ margin: 0 }}>{analysisNote}</p> : null}
          {candidates && candidates.length === 0 ? (
            <p style={{ margin: 0 }}>{m.importPanel.historyNone}</p>
          ) : null}
          {candidates && candidates.length > 0 ? (
            <>
              <div style={{ display: 'grid', gap: 'var(--fh-space-2)' }}>
                {candidates.map((c, i) => (
                  <div key={`${c.name}-${i}`} style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'flex-start' }}>
                    <Checkbox
                      ariaLabel={fmt(m.importPanel.historyPickAria, { name: c.name })}
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
                            label={m.importPanel.historyNameLabel}
                            value={c.name}
                            onChange={(name) =>
                              setCandidates(
                                candidates.map((x, xi) => (xi === i ? { ...x, name } : x)),
                              )
                            }
                          />
                        </div>
                        <span>
                          {fmt(m.importPanel.historySeen, { count: c.count })}
                          {c.state === 'done' ? ` - ${m.importPanel.historyImportedState}` : ''}
                          {c.state === 'error' ? ` - ${fmt(m.importPanel.historyFailedState, { error: c.error ?? '' })}` : ''}
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
                  {m.importPanel.historyImportSelected}
                </Button>
              </div>
            </>
          ) : null}
        </div>
      </div>
    </Card>
  );
}
