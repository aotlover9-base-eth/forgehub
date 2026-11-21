import { useNavigate } from 'react-router-dom';
import { EmptyState } from '../design';

/** The shared 404: used for unknown routes and for things
 * that must not reveal whether they exist (private snippets
 * use their own instance of the same wording). */
export default function NotFoundPage() {
  const navigate = useNavigate();
  return (
    <EmptyState
      icon="search"
      title="Not found"
      message="This page does not exist, or it is private."
      titleLevel={1}
      actionLabel="Go home"
      onAction={() => navigate('/')}
    />
  );
}
