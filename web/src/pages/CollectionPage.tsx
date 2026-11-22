import { useState } from 'react';
import { fmt, m } from '../i18n/messages';
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
  useCreateShareLink,
  useDeleteCollection,
  useMe,
  usePatchCollection,
  useRemoveFromCollection,
  useReorderCollection,
  useRevokeShareLink,
  useShareLinks,
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
        title={notFound ? m.collection.notFoundTitle : m.collection.loadFailedTitle}
        message={
          notFound
            ? m.collection.notFoundMessage
            : m.collection.loadFailedMessage
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
      toast({ message: m.collection.addRefInvalid });
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
          toast({ message: fmt(m.collection.addedToast, { ref: refr }) });
        },
        onError: (error) => toast({ message: error.uiMessage }),
      },
    );
  };

  return (
    <div style={{ display: 'grid', gap: 'var(--fh-space-4)' }}>
      <PageHeader
        title={data.name}
        subtitle={fmt(m.collection.subtitle, { owner: data.owner, count: data.item_count, word: data.item_count === 1 ? m.library.savedOne : m.library.savedMany, descriptionPart: data.description ? fmt(m.collection.descriptionPart, { description: data.description }) : '' })}
        actions={<StatusChip visibility={data.visibility} />}
      />

      <Card>
        <div style={{ display: 'grid', gap: 'var(--fh-space-2)', padding: 'var(--fh-space-4)' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--fh-space-2)' }}>
            <h2 style={{ margin: 0, flex: 1 }}>{m.collection.runFromTerminal}</h2>
            <CopyIconButton
              text={runCommand}
              label={m.collection.copyRun}
              onCopied={() => toast({ message: fmt(m.collection.copiedCommand, { command: runCommand }) })}
            />
          </div>
          <CodeBlock code={runCommand} label={m.collection.runCodeLabel} />
          <p style={{ margin: 0 }}>
            {m.collection.trustNote}
          </p>
        </div>
      </Card>

      {isOwner ? <ShareLinksCard owner={data.owner} name={data.name} /> : null}

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
                label={m.collection.addLabel}
                value={addRef}
                onChange={setAddRef}
                supportingText={m.collection.addHint}
              />
            </div>
            <Button variant="tonal" icon="add" onClick={submitAdd} disabled={addItem.isPending}>
              {m.collection.add}
            </Button>
            <span style={{ flexBasis: '100%', height: 0 }} />
            <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center', flexWrap: 'wrap' }}>
              <span>{m.collection.visibilityLabel}</span>
              <md-chip-set aria-label={m.collection.visibilityAria}>
                {VISIBILITIES.map((v) => (
                  <md-filter-chip
                    key={v}
                    label={v}
                    selected={data.visibility === v || undefined}
                    onClick={() =>
                      patch.mutate(
                        { owner: data.owner, name: data.name, visibility: v },
                        {
                          onSuccess: () => toast({ message: fmt(m.collection.nowVisibility, { name: data.name, visibility: v }) }),
                          onError: (error) => toast({ message: error.uiMessage }),
                        },
                      )
                    }
                  />
                ))}
              </md-chip-set>
              <span style={{ flex: 1 }} />
              <Button variant="text" icon="delete" onClick={() => setDeleteOpen(true)}>
                {m.collection.deleteCollection}
              </Button>
            </div>
          </div>
        </Card>
      ) : null}

      <Card>
        {data.items.length === 0 ? (
          <EmptyState
            icon="playlist_add"
            title={m.collection.emptyTitle}
            message={
              hiddenCount > 0
                ? m.collection.emptyAllPrivate
                : isOwner
                  ? m.collection.emptyOwner
                  : m.collection.emptyOther
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
                      label={fmt(m.collection.moveUp, { name: item.name })}
                      onClick={() => move(index, -1)}
                      disabled={index === 0 || reorder.isPending}
                    />
                    <IconButton
                      icon="arrow_downward"
                      label={fmt(m.collection.moveDown, { name: item.name })}
                      onClick={() => move(index, 1)}
                      disabled={index === data.items.length - 1 || reorder.isPending}
                    />
                    <IconButton
                      icon="close"
                      label={fmt(m.collection.removeLabel, { name: item.name })}
                      onClick={() =>
                        removeItem.mutate(
                          {
                            owner: data.owner,
                            name: data.name,
                            snippetOwner: item.owner,
                            snippetName: item.name,
                          },
                          {
                            onSuccess: () => toast({ message: fmt(m.collection.removedToast, { name: item.name }) }),
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
            {fmt(m.collection.hiddenMore, { count: hiddenCount, verb: hiddenCount === 1 ? m.collection.hiddenIs : m.collection.hiddenAre })}
          </p>
        ) : null}
      </Card>

      <ConfirmDialog
        open={deleteOpen}
        headline={fmt(m.collection.deleteHeadline, { name: data.name })}
        content={m.collection.deleteContent}
        confirmLabel={m.collection.deleteConfirm}
        danger
        onCancel={() => setDeleteOpen(false)}
        onConfirm={() =>
          deleteCollection.mutate(
            { owner: data.owner, name: data.name },
            {
              onSuccess: () => {
                toast({ message: fmt(m.collection.deletedToast, { name: data.name }) });
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

const EXPIRY_CHOICES = [1, 7, 30];

/**
 * Share links (Peak C6, ADR 0011): the owner can mint an
 * expiring, revocable read-only link. The full URL is shown
 * once, right after minting - the server stores only a hash,
 * so it can never be displayed again.
 */
function ShareLinksCard({ owner, name }: { owner: string; name: string }) {
  const toast = useToast();
  const links = useShareLinks(owner, name, true);
  const create = useCreateShareLink(owner, name);
  const revoke = useRevokeShareLink(owner, name);
  const [days, setDays] = useState(7);
  const [freshUrl, setFreshUrl] = useState('');

  const mint = () => {
    create.mutate(
      { expires_in_days: days },
      {
        onSuccess: (link) => {
          setFreshUrl(`${window.location.origin}${link.url}`);
          toast({ message: m.collection.shareCreatedToast });
        },
        onError: (error) => toast({ message: error.uiMessage }),
      },
    );
  };

  return (
    <Card>
      <div style={{ display: 'grid', gap: 'var(--fh-space-3)', padding: 'var(--fh-space-4)' }}>
        <h2 style={{ margin: 0 }}>{m.collection.shareTitle}</h2>
        <p style={{ margin: 0 }}>
          {m.collection.shareBody}
        </p>
        <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center', flexWrap: 'wrap' }}>
          <span>{m.collection.expiresAfter}</span>
          <md-chip-set aria-label={m.collection.expiryAria}>
            {EXPIRY_CHOICES.map((d) => (
              <md-filter-chip
                key={d}
                label={d === 1 ? m.collection.dayOne : fmt(m.collection.daysMany, { days: d })}
                selected={days === d || undefined}
                onClick={() => setDays(d)}
              />
            ))}
          </md-chip-set>
          <Button variant="tonal" icon="link" onClick={mint} disabled={create.isPending}>
            {m.collection.createShareLink}
          </Button>
        </div>
        {freshUrl ? (
          <div style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center' }}>
            <code style={{ flex: 1, minWidth: 0, overflowWrap: 'anywhere' }}>{freshUrl}</code>
            <CopyIconButton
              text={freshUrl}
              label={m.collection.copyShareLink}
              onCopied={() => toast({ message: m.collection.shareCopied })}
            />
          </div>
        ) : null}
        {links.data && links.data.links.length > 0 ? (
          <div style={{ display: 'grid', gap: 'var(--fh-space-2)' }}>
            {links.data.links.map((link) => (
              <div
                key={link.id}
                style={{ display: 'flex', gap: 'var(--fh-space-2)', alignItems: 'center', flexWrap: 'wrap' }}
              >
                <code>fgl_{link.prefix}...</code>
                <span>
                  {link.active
                    ? fmt(m.collection.linkActive, { date: link.expires_at.slice(0, 10) })
                    : link.revoked_at
                      ? m.collection.linkRevoked
                      : fmt(m.collection.linkExpired, { date: link.expires_at.slice(0, 10) })}
                </span>
                <span style={{ flex: 1 }} />
                {link.active ? (
                  <Button
                    variant="text"
                    icon="link_off"
                    onClick={() =>
                      revoke.mutate(link.id, {
                        onSuccess: () => toast({ message: m.collection.shareRevokedToast }),
                        onError: (error) => toast({ message: error.uiMessage }),
                      })
                    }
                    disabled={revoke.isPending}
                  >
                    {m.collection.revoke}
                  </Button>
                ) : null}
              </div>
            ))}
          </div>
        ) : null}
      </div>
    </Card>
  );
}
