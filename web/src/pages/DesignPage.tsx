import { useState } from 'react';
import { fmt, m } from '../i18n/messages';
import {
  Button,
  Card,
  Chip,
  CodeBlock,
  ConfirmDialog,
  EmptyState,
  ErrorState,
  Fab,
  IconButton,
  ListItem,
  Menu,
  PageHeader,
  SearchBar,
  Skeleton,
  StatusChip,
  Checkbox,
  DiffView,
  Radio,
  SelectField,
  Switch,
  Tabs,
  TextField,
  useToast,
} from '../design';

/**
 * The design-system review page (dev builds only, /__design):
 * every component and state in one place, in whichever theme
 * (light, dark, system) is active.
 */
export default function DesignPage() {
  const toast = useToast();
  const [query, setQuery] = useState('');
  const [field, setField] = useState('');
  const [tab, setTab] = useState(0);
  const [wrap, setWrap] = useState(true);
  const [scope, setScope] = useState(false);
  const [choice, setChoice] = useState('system');
  const [expiry, setExpiry] = useState('never');
  const [dialog, setDialog] = useState(false);
  const [dangerDialog, setDangerDialog] = useState(false);
  return (
    <div>
      <PageHeader
        title={m.designPage.title}
        subtitle={m.designPage.subtitle}
        actions={
          <>
            <Button variant="outlined" onClick={() => setDialog(true)}>
              {m.designPage.dialog}
            </Button>
            <Button variant="filled" onClick={() => toast({ message: m.designPage.toastSaved })}>
              {m.designPage.toast}
            </Button>
          </>
        }
      />

      <h2>{m.designPage.buttons}</h2>
      <p style={{ display: 'flex', gap: 'var(--fh-space-2)', flexWrap: 'wrap' }}>
        <Button variant="filled" icon="add">
          {m.designPage.filled}
        </Button>
        <Button variant="tonal">{m.designPage.tonal}</Button>
        <Button variant="outlined">{m.designPage.outlined}</Button>
        <Button variant="text">{m.designPage.text}</Button>
        <Button variant="filled" disabled>
          {m.designPage.disabled}
        </Button>
        <IconButton icon="star" label={m.designPage.star} />
        <Fab icon="add" label={m.designPage.createSnippet} />
      </p>

      <h2>{m.designPage.chipsTitle}</h2>
      <p style={{ display: 'flex', gap: 'var(--fh-space-2)', flexWrap: 'wrap', alignItems: 'center' }}>
        <Chip label="docker" />
        <Chip label="ubuntu" icon="tag" />
        <StatusChip visibility="public" />
        <StatusChip visibility="unlisted" />
        <StatusChip visibility="private" />
      </p>

      <h2>{m.designPage.inputs}</h2>
      <SearchBar value={query} onChange={setQuery} placeholder={m.designPage.searchPlaceholder} />
      <div style={{ height: 'var(--fh-space-4)' }} />
      <TextField label={m.designPage.nameLabel} value={field} onChange={setField} supportingText={m.designPage.nameHint} />
      <div style={{ height: 'var(--fh-space-3)' }} />
      <TextField label={m.designPage.errorLabel} value={field} onChange={setField} errorText={m.designPage.errorText} />
      <div style={{ height: 'var(--fh-space-3)' }} />
      <TextField label={m.designPage.description} value={field} onChange={setField} multiline rows={3} />

      <h2>{m.designPage.selectionTitle}</h2>
      <div style={{ display: 'grid', gap: 'var(--fh-space-2)' }}>
        <Switch checked={wrap} onChange={setWrap} label={m.designPage.switchLabel} />
        <Checkbox checked={scope} onChange={setScope} label={m.designPage.checkboxLabel} />
        <Radio
          name="gallery-choice"
          value="system"
          checked={choice === 'system'}
          onChange={setChoice}
          label={
            <>
              <strong>{m.designPage.radioSystemStrong}</strong> {m.designPage.radioSystemRest}
            </>
          }
        />
        <Radio
          name="gallery-choice"
          value="dark"
          checked={choice === 'dark'}
          onChange={setChoice}
          label={
            <>
              <strong>{m.designPage.radioDarkStrong}</strong> {m.designPage.radioDarkRest}
            </>
          }
        />
        <SelectField
          label={m.designPage.expires}
          value={expiry}
          onChange={setExpiry}
          options={[
            { value: 'never', label: m.designPage.expiryNever },
            { value: '30', label: m.designPage.expiry30 },
            { value: '90', label: m.designPage.expiry90 },
          ]}
        />
      </div>

      <h2>{m.designPage.diff}</h2>
      <DiffView diff={'--- a/sys-info\n+++ b/sys-info\n@@ -1,2 +1,2 @@\n-echo old\n+echo new'} />

      <h2>{m.designPage.tabsTitle}</h2>
      <Tabs
        ariaLabel={m.designPage.tabsAria}
        tabs={[
          { id: 'one', label: m.designPage.tabOverview },
          { id: 'two', label: m.designPage.tabVersions },
          { id: 'three', label: m.designPage.tabSettings },
        ]}
        activeIndex={tab}
        onChange={setTab}
      />
      <p>
        {m.designPage.activeTab} {tab}{' '}
        <Menu
          buttonLabel={m.designPage.moreActions}
          items={[
            { id: 'edit', label: m.designPage.menuEdit, icon: 'edit' },
            { id: 'fork', label: m.designPage.menuFork, icon: 'call_split' },
            { id: 'delete', label: m.designPage.menuDelete, icon: 'delete' },
          ]}
          onSelect={(id) => toast({ message: fmt(m.designPage.menuToast, { id }) })}
        />
      </p>

      <h2>{m.designPage.cardsTitle}</h2>
      <div style={{ display: 'grid', gap: 'var(--fh-space-3)' }}>
        <Card variant="filled">
          <strong>{m.designPage.filledCard}</strong>
          <md-list>
            <ListItem headline="ubuntu-setup" supportingText="Update packages and install basics" trailing={<StatusChip visibility="public" />} />
            <ListItem headline="docker-clean" supportingText="Remove stopped containers" trailing={<StatusChip visibility="private" />} />
          </md-list>
        </Card>
        <Card variant="outlined">
          <strong>{m.designPage.outlinedCard}</strong>
          <p>{m.designPage.outlinedCardBody}</p>
        </Card>
        <Card variant="elevated">
          <strong>{m.designPage.elevatedCard}</strong>
          <p>{m.designPage.elevatedCardBody}</p>
        </Card>
      </div>

      <h2>{m.designPage.code}</h2>
      <CodeBlock
        label={m.designPage.codeLabel}
        lineNumbers
        code={'sudo apt-get update\nsudo apt-get install -y curl git\necho "a very long line that must wrap instead of being truncated so nothing can hide beyond the edge of the screen ever"'}
      />

      <h2>{m.designPage.states}</h2>
      <Card variant="outlined">
        <Skeleton lines={3} />
      </Card>
      <EmptyState icon="library_books" title={m.designPage.emptyTitle} message={m.designPage.emptyMessage} actionLabel={m.designPage.createSnippet} onAction={() => toast({ message: m.designPage.createDemo })} />
      <ErrorState title={m.designPage.errorTitle} message={m.designPage.errorMessage} retryLabel={m.designPage.retry} onRetry={() => toast({ message: m.designPage.retryingDemo })} />

      <ConfirmDialog
        open={dialog}
        headline={m.designPage.makePublicHeadline}
        content={m.designPage.makePublicContent}
        confirmLabel={m.designPage.makePublicConfirm}
        onConfirm={() => {
          setDialog(false);
          toast({ message: m.designPage.visibilityUpdatedDemo });
        }}
        onCancel={() => setDialog(false)}
      />
      <ConfirmDialog
        open={dangerDialog}
        headline={m.designPage.deleteHeadline}
        content={m.designPage.deleteContent}
        confirmLabel={m.designPage.deleteConfirm}
        danger
        onConfirm={() => setDangerDialog(false)}
        onCancel={() => setDangerDialog(false)}
      />
      <p>
        <Button variant="text" onClick={() => setDangerDialog(true)}>
          {m.designPage.openDanger}
        </Button>
      </p>
    </div>
  );
}
