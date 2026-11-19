import { Component, type ReactNode } from 'react';
import { ErrorState } from './design';

/** App-level error boundary: a render crash becomes a calm
 * error page with a reload, never a white screen. */
export class ErrorBoundary extends Component<
  { children: ReactNode },
  { failed: boolean }
> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  render() {
    if (this.state.failed) {
      return (
        <ErrorState
          title="Something went wrong"
          message="The page hit an unexpected problem. Reloading usually fixes it; your snippets are safe on the server."
          retryLabel="Reload"
          onRetry={() => window.location.reload()}
        />
      );
    }
    return this.props.children;
  }
}
