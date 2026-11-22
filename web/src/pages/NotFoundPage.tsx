import { useNavigate } from 'react-router-dom';
import { EmptyState } from '../design';
import { m } from '../i18n/messages';

/** The shared 404: used for unknown routes and for things
 * that must not reveal whether they exist (private snippets
 * use their own instance of the same wording). */
export default function NotFoundPage() {
  const navigate = useNavigate();
  return (
    <EmptyState
      icon="search"
      title={m.notFound.title}
      message={m.notFound.message}
      titleLevel={1}
      actionLabel={m.notFound.goHome}
      onAction={() => navigate('/')}
    />
  );
}
