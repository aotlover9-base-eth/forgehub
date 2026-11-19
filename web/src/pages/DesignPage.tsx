import { useState } from 'react';
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
  Tabs,
  TextField,
  useToast,
} from '../design';

/**
 * The design-system review page (dev builds only, /__design):
 * every component and state in one place. Toggle the theme
 * from the app bar to review light and dark.
 */
export default function DesignPage() {
  const toast = useToast();
  const [query, setQuery] = useState('');
  const [field, setField] = useState('');
  const [tab, setTab] = useState(0);
  const [dialog, setDialog] = useState(false);
  const [dangerDialog, setDangerDialog] = useState(false);
  return (
    <div>
      <PageHeader
        title="Design system"
        subtitle="Material You 3 · one seed colour · tokens only"
        actions={
          <>
            <Button variant="outlined" onClick={() => setDialog(true)}>
              Dialog
            </Button>
            <Button variant="filled" onClick={() => toast({ message: 'Saved to your library' })}>
              Toast
            </Button>
          </>
        }
      />

      <h2>Buttons</h2>
      <p style={{ display: 'flex', gap: 'var(--fh-space-2)', flexWrap: 'wrap' }}>
        <Button variant="filled" icon="add">
          Filled
        </Button>
        <Button variant="tonal">Tonal</Button>
        <Button variant="outlined">Outlined</Button>
        <Button variant="text">Text</Button>
        <Button variant="filled" disabled>
          Disabled
        </Button>
        <IconButton icon="star" label="Star" />
        <Fab icon="add" label="Create snippet" />
      </p>

      <h2>Chips and status</h2>
      <p style={{ display: 'flex', gap: 'var(--fh-space-2)', flexWrap: 'wrap', alignItems: 'center' }}>
        <Chip label="docker" />
        <Chip label="ubuntu" icon="tag" />
        <StatusChip visibility="public" />
        <StatusChip visibility="unlisted" />
        <StatusChip visibility="private" />
      </p>

      <h2>Inputs</h2>
      <SearchBar value={query} onChange={setQuery} placeholder="Search snippets" />
      <div style={{ height: 'var(--fh-space-4)' }} />
      <TextField label="Snippet name" value={field} onChange={setField} supportingText="Lowercase, digits and hyphens" />
      <div style={{ height: 'var(--fh-space-3)' }} />
      <TextField label="With an error" value={field} onChange={setField} errorText="That name is taken" />
      <div style={{ height: 'var(--fh-space-3)' }} />
      <TextField label="Description" value={field} onChange={setField} multiline rows={3} />

      <h2>Tabs and menu</h2>
      <Tabs
        ariaLabel="Example tabs"
        tabs={[
          { id: 'one', label: 'Overview' },
          { id: 'two', label: 'Versions' },
          { id: 'three', label: 'Settings' },
        ]}
        activeIndex={tab}
        onChange={setTab}
      />
      <p>
        Active tab: {tab}{' '}
        <Menu
          buttonLabel="More actions"
          items={[
            { id: 'edit', label: 'Edit', icon: 'edit' },
            { id: 'fork', label: 'Fork', icon: 'call_split' },
            { id: 'delete', label: 'Delete', icon: 'delete' },
          ]}
          onSelect={(id) => toast({ message: `Menu: ${id}` })}
        />
      </p>

      <h2>Cards and list</h2>
      <div style={{ display: 'grid', gap: 'var(--fh-space-3)' }}>
        <Card variant="filled">
          <strong>Filled card</strong>
          <md-list>
            <ListItem headline="ubuntu-setup" supportingText="Update packages and install basics" trailing={<StatusChip visibility="public" />} />
            <ListItem headline="docker-clean" supportingText="Remove stopped containers" trailing={<StatusChip visibility="private" />} />
          </md-list>
        </Card>
        <Card variant="outlined">
          <strong>Outlined card</strong>
          <p>Neutral surfaces, one accent for actions.</p>
        </Card>
        <Card variant="elevated">
          <strong>Elevated card</strong>
          <p>Elevation 1 shadow from the token set.</p>
        </Card>
      </div>

      <h2>Code</h2>
      <CodeBlock
        label="ubuntu-setup · v3"
        lineNumbers
        code={'sudo apt-get update\nsudo apt-get install -y curl git\necho "a very long line that must wrap instead of being truncated so nothing can hide beyond the edge of the screen ever"'}
      />

      <h2>States</h2>
      <Card variant="outlined">
        <Skeleton lines={3} />
      </Card>
      <EmptyState icon="library_books" title="No snippets yet" message="Record or write your first snippet and it will show up here." actionLabel="Create snippet" onAction={() => toast({ message: 'Create (demo)' })} />
      <ErrorState title="Could not load" message="The server did not respond. Check your connection and try again." retryLabel="Retry" onRetry={() => toast({ message: 'Retrying (demo)' })} />

      <ConfirmDialog
        open={dialog}
        headline="Make public?"
        content="Anyone will be able to read and run this snippet. Review it for secrets first."
        confirmLabel="Make public"
        onConfirm={() => {
          setDialog(false);
          toast({ message: 'Visibility updated (demo)' });
        }}
        onCancel={() => setDialog(false)}
      />
      <ConfirmDialog
        open={dangerDialog}
        headline="Delete this snippet?"
        content="It will be removed from your library. This cannot be undone after 30 days."
        confirmLabel="Delete"
        danger
        onConfirm={() => setDangerDialog(false)}
        onCancel={() => setDangerDialog(false)}
      />
      <p>
        <Button variant="text" onClick={() => setDangerDialog(true)}>
          Open the danger dialog
        </Button>
      </p>
    </div>
  );
}
