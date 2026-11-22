import { Button, Card, Chip, ListItem, PageHeader, StatusChip, TextField } from '../design';
import { m } from '../i18n/messages';

/**
 * The RTL layout test page (Peak C9): representative
 * ForgeHub UI rendered inside a dir="rtl" subtree with
 * sample Arabic text, so the mirrored layout (flex
 * direction, text alignment, logical CSS properties) can
 * be checked by eye and by the rtl test. The message
 * catalogue is English-only for now; the samples here are
 * test content, labelled as such, living in the catalogue
 * like every other string.
 */
export default function RtlTestPage() {
  return (
    <div style={{ display: 'grid', gap: 'var(--fh-space-4)' }}>
      <PageHeader title={m.rtlTest.title} subtitle={m.rtlTest.subtitle} />
      <div dir="rtl" aria-label={m.rtlTest.panelLabel}>
        <Card variant="outlined">
          <div style={{ display: 'grid', gap: 'var(--fh-space-3)', padding: 'var(--fh-space-4)' }}>
            <h2 style={{ margin: 0 }}>{m.rtlTest.sampleHeading}</h2>
            <p style={{ margin: 0 }}>{m.rtlTest.sampleBody}</p>
            <md-list>
              <ListItem
                headline={m.rtlTest.sampleItemOne}
                supportingText="ubuntu-setup"
                trailing={<StatusChip visibility="public" />}
              />
              <ListItem
                headline={m.rtlTest.sampleItemTwo}
                supportingText="docker-clean"
                trailing={<StatusChip visibility="private" />}
              />
            </md-list>
            <div style={{ display: 'flex', gap: 'var(--fh-space-2)', flexWrap: 'wrap' }}>
              <Chip label="docker" />
              <Chip label="ubuntu" />
            </div>
            <TextField label={m.rtlTest.sampleFieldLabel} value="" onChange={() => undefined} />
            <p style={{ display: 'flex', gap: 'var(--fh-space-2)', margin: 0 }}>
              <Button variant="filled">{m.rtlTest.samplePrimary}</Button>
              <Button variant="text">{m.rtlTest.sampleSecondary}</Button>
            </p>
          </div>
        </Card>
      </div>
      <p style={{ margin: 0 }}>{m.rtlTest.ltrNote}</p>
    </div>
  );
}
