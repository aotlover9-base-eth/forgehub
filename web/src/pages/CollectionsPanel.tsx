import { useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { Button, Card, StatusChip, TextField, useToast } from '../design';
import { useCreateCollection, useMe, useMyCollections } from '../api/hooks';
import type { Visibility } from '../api/types';
import { fmt, m } from '../i18n/messages';

const VISIBILITIES: Visibility[] = ['private', 'unlisted', 'public'];

/**
 * The Library's Collections section (Peak C1): the user's
 * collections with item counts, plus a create form. Full
 * management (items, order, visibility) lives on the
 * collection page itself.
 */
export default function CollectionsPanel() {
  const collections = useMyCollections();
  const create = useCreateCollection();
  const me = useMe();
  const toast = useToast();
  const navigate = useNavigate();
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [visibility, setVisibility] = useState<Visibility>('private');

  const rows = collections.data?.collections ?? [];
  const username = me.data?.username ?? '';

  const submit = () => {
    const trimmed = name.trim();
    if (!trimmed) {
      toast({ message: m.collectionsPanel.nameRequired });
      return;
    }
    create.mutate(
      { name: trimmed, description: description.trim(), visibility },
      {
        onSuccess: (created) => {
          setName('');
          setDescription('');
          toast({ message: fmt(m.collectionsPanel.created, { name: created.name }) });
          navigate(`/collections/${created.owner}/${created.name}`);
        },
        onError: (error) => toast({ message: error.uiMessage }),
      },
    );
  };

  return (
    <Card>
      <div style={{ display: 'grid', gap: 'var(--fh-space-3)', padding: 'var(--fh-space-4)' }}>
        <h2 style={{ margin: 0 }}>{m.collectionsPanel.title}</h2>
        {collections.isPending ? (
          <p style={{ margin: 0 }}>{m.collectionsPanel.loading}</p>
        ) : rows.length === 0 ? (
          <p style={{ margin: 0 }}>
            {m.collectionsPanel.empty}
          </p>
        ) : (
          <div style={{ display: 'grid', gap: 'var(--fh-space-2)' }}>
            {rows.map((c) => (
              <div
                key={c.name}
                style={{ display: 'flex', alignItems: 'center', gap: 'var(--fh-space-3)' }}
              >
                <Link to={`/collections/${username}/${c.name}`} style={{ flex: 1, minWidth: 0 }}>
                  {c.name}
                  {c.description ? ` - ${c.description}` : ''}
                </Link>
                <span>
                  {fmt(c.item_count === 1 ? m.collectionsPanel.items : m.collectionsPanel.itemsPlural, { count: c.item_count })}
                </span>
                <StatusChip visibility={c.visibility} />
              </div>
            ))}
          </div>
        )}
        <div
          style={{
            display: 'flex',
            gap: 'var(--fh-space-2)',
            alignItems: 'flex-end',
            flexWrap: 'wrap',
          }}
        >
          <div style={{ flex: 1, minWidth: 160 }}>
            <TextField label={m.collectionsPanel.newName} value={name} onChange={setName} supportingText={m.collectionsPanel.newNameHint} />
          </div>
          <div style={{ flex: 2, minWidth: 200 }}>
            <TextField label={m.collectionsPanel.descriptionOptional} value={description} onChange={setDescription} />
          </div>
          <md-chip-set aria-label={m.collectionsPanel.visibilityLabel}>
            {VISIBILITIES.map((v) => (
              <md-filter-chip
                key={v}
                label={v}
                selected={visibility === v || undefined}
                onClick={() => setVisibility(v)}
              />
            ))}
          </md-chip-set>
          <Button variant="tonal" icon="add" onClick={submit} disabled={create.isPending}>
            {m.collectionsPanel.create}
          </Button>
        </div>
      </div>
    </Card>
  );
}
