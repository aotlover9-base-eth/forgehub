import { useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import {
  Button,
  Card,
  CodeBlock,
  ConfirmDialog,
  CopyIconButton,
  EmptyState,
  ErrorState,
  IconButton,
  PageHeader,
  Skeleton,
  StatusChip,
  TextField,
  useToast,
} from '../design';
import { ApiError } from '../api/client';
import {
  useAddToCollection,
  useCollection,
  useDeleteCollection,
  useMe,
  usePatchCollection,
  useRemoveFromCollection,
  useReorderCollection,
} from '../api/hooks';
import type { Visibility } from '../api/types';

const VISIBILITIES: Visibility[] = ['private', 'unlisted', 'public'];

/**
 * A collection (Peak C1): an ordered runbook of snippets.
 * The server filters items through the snippet read rules,
 * so this page only ever shows what the viewer may open.
 * Owners get the management controls (add, remove, reorder,
 * visibility, delete); everyone else gets the read view.
 */
export default function CollectionPage() {
  const { owner = '', name = '' } = useParams();
  const collection = useCollection(owner, name);
  const me = useMe();
  const toast = useToast();
  const navigate = useNavigate();
  const addItem = useAddToCollection();
  const removeItem = useRemoveFromCollection();
  const reorder = useReorderCollection();
  const patch = usePatchCollection();
  const deleteCollection = useDeleteCollection();
  const [addRef, setAddRef] = useState('');
  const [deleteOpen, setDeleteOpen] = useState(false);

  if (collection.isPending) {
    return (
      <div style={{ display: 'grid', gap: 'var(--fh-space-3)' }}>
        <Skeleton lines={2} />
        <Skeleton lines={6} />
      </div>
    );
  }
  if (collection.error || !collection.data) {
    const notFound =
      collection.error instanceof ApiError && collection.error.status === 404;
    return (
      <ErrorState
        title={notFound ? 'Collection not found' : 'Could not load this collection'}
        message={
          notFound
            ? 'It may be private, renamed, or deleted. Private collections are only visible to their owner.'
            : 'Please try again in a moment.'
        }
      />
    );
  }

  const data = collection.data;
  const isOwner = me.data?.username === data.owner;
  const runCommand = `fg run ${data.owner}/${data.name}`;
  const hiddenCount = (data.total_items ?? data.items.length) - data.items.length;

  const move = (index: number, delta: number) => {
    const items = [...data.items];
    const target = index + delta;
    if (target < 0 || target >= items.length) return;
    const [moved] = items.splice(index, 1);
    items.splice(target, 0, moved);
    reorder.mutate(
      {
        owner: data.owner,
        name: data.name,
        items: items.map((i) => `${i.owner}/${i.name}`),
      },
      { onError: (error) => toast({ message: error.uiMessage }) },
    );
  };

  const submitAdd = () => {
    const refr = addRef.trim();
    const slash = refr.indexOf('/');
    if (slash <= 0 || slash === refr.length - 1) {
      toast({ message: 'Enter the snippet as owner/name' });
      return;
    }
    addItem.mutate(
      {
        owner: data.owner,
        name: data.name,
        snippetOwner: refr.slice(0, slash),
        snippetName: refr.slice(slash + 1),
      },
      {
        onSuccess: () => {
          setAddRef('');
          toast({ message: `Added ${refr}` });
        },
        onError: (error) => toast({ message: error.uiMessage }),
      },
    );
  };

  return (
    <div style={{ display: 'grid', gap: 'var(--fh-space-4)' }}>
      <PageHeader
        title={data.name}
        subtitle={`Collection by ${data.owner} - ${data.item_count} snippet${data.item_count === 1 ? '' : 's'}${data.description ? ` - ${data.description}` : ''}`}
        actions={<StatusChip visibility={data.visibility} />}
      />

      <Card>
        <div style={{ display: 'grid', gap: 'var(--fh-space-2)', padding: 'var(--fh-space-4)' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--fh-space-2)' }}>
            <h2 style={{ margin: 0, flex: 1 }}>Run it from your terminal</h2>
            <CopyIconButton
              text={runCommand}
              label="Copy run command"
              onCopied={() => toast({ message: `Copied: ${runCommand}` })}
            />
          </div>
          <CodeBlock code={runCommand} label="Run this collection" />
          <p style={{ margin: 0 }}>
            Every snippet is confirmed before it runs, in order - the same trust prompts as
            running each one directly.
          </p>
        </div>
      </Card>

      {isOwner ? (
        <Card>
          <div
            style={{
              display: 'flex',
              gap: 'var(--fh-space-3)',
              alignItems: 'flex-end',
              flexWrap: 'wrap',
              padding: 'var(--fh-space-4)',
            }}
          >
            <div style={{ flex: 1, minWidth: 220 }}>
              <TextField
                label="Add a snippet (owner/name)"
                value={addRef}
                onChange={setAddRef}
                supportingText="Any snippet you can open, e.g. anna/restart-api"
              />
            </div>
            <Button variant="tonal" icon="add" onClick={submitAdd} disabled={addItem.isPending}>
              Add
            </Button>
            <span style={{ flexBasis: '100%', height: 0 }} />
            <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center', flexWrap: 'wrap' }}>
              <span>Visibility:</span>
              <md-chip-set aria-label="Collection visibility">
                {VISIBILITIES.map((v) => (
                  <md-filter-chip
                    key={v}
                    label={v}
                    selected={data.visibility === v || undefined}
                    onClick={() =>
                      patch.mutate(
                        { owner: data.owner, name: data.name, visibility: v },
                        {
                          onSuccess: () => toast({ message: `${data.name} is now ${v}` }),
                          onError: (error) => toast({ message: error.uiMessage }),
                        },
                      )
                    }
                  />
                ))}
              </md-chip-set>
              <span style={{ flex: 1 }} />
              <Button variant="text" icon="delete" onClick={() => setDeleteOpen(true)}>
                Delete collection
              </Button>
            </div>
          </div>
        </Card>
      ) : null}

      <Card>
        {data.items.length === 0 ? (
          <EmptyState
            icon="playlist_add"
            title="No snippets to show"
            message={
              hiddenCount > 0
                ? 'Every item in this collection is private to its owner.'
                : isOwner
                  ? 'Add your first snippet above - use any owner/name you can open.'
                  : 'This collection is empty.'
            }
          />
        ) : (
          <md-list>
            {data.items.map((item, index) => (
              <div
                key={`${item.owner}/${item.name}`}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 'var(--fh-space-3)',
                  padding: 'var(--fh-space-2) var(--fh-space-4)',
                }}
              >
                <span aria-hidden="true">{index + 1}.</span>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <Link to={`/${item.owner}/${item.name}`}>
                    {item.owner}/{item.name}
                  </Link>
                  {item.description ? <div>{item.description}</div> : null}
                </div>
                <span>v{item.latest_version}</span>
                <StatusChip visibility={item.visibility} />
                {isOwner ? (
                  <>
                    <IconButton
                      icon="arrow_upward"
                      label={`Move ${item.name} up`}
                      onClick={() => move(index, -1)}
                      disabled={index === 0 || reorder.isPending}
                    />
                    <IconButton
                      icon="arrow_downward"
                      label={`Move ${item.name} down`}
                      onClick={() => move(index, 1)}
                      disabled={index === data.items.length - 1 || reorder.isPending}
                    />
                    <IconButton
                      icon="close"
                      label={`Remove ${item.name} from the collection`}
                      onClick={() =>
                        removeItem.mutate(
                          {
                            owner: data.owner,
                            name: data.name,
                            snippetOwner: item.owner,
                            snippetName: item.name,
                          },
                          {
                            onSuccess: () => toast({ message: `Removed ${item.name}` }),
                            onError: (error) => toast({ message: error.uiMessage }),
                          },
                        )
                      }
                    />
                  </>
                ) : null}
              </div>
            ))}
          </md-list>
        )}
        {hiddenCount > 0 ? (
          <p style={{ padding: '0 var(--fh-space-4) var(--fh-space-3)', margin: 0 }}>
            {hiddenCount} more item{hiddenCount === 1 ? ' is' : 's are'} not readable by you
            and hidden.
          </p>
        ) : null}
      </Card>

      <ConfirmDialog
        open={deleteOpen}
        headline={`Delete ${data.name}?`}
        content="The collection is removed. The snippets in it are not deleted."
        confirmLabel="Delete"
        danger
        onCancel={() => setDeleteOpen(false)}
        onConfirm={() =>
          deleteCollection.mutate(
            { owner: data.owner, name: data.name },
            {
              onSuccess: () => {
                toast({ message: `${data.name} deleted` });
                navigate('/dashboard');
              },
              onError: (error) => toast({ message: error.uiMessage }),
              onSettled: () => setDeleteOpen(false),
            },
          )
        }
      />
    </div>
  );
}
