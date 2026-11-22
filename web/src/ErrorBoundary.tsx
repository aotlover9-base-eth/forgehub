import { fmt, m } from './i18n/messages';
import { Component, type ReactNode } from 'react';
import { ErrorState } from './design';

/** App-level error boundary: a render crash becomes a calm
 * error page with a reload, never a white screen. */
export class ErrorBoundary extends Component<
  { children: ReactNode },
  { failed: boolean; refId: string }
> {
  state = { failed: false, refId: '' };

  static getDerivedStateFromError() {
    // A short reference the user can quote in a bug report.
    // Never a stack trace - that stays in the console.
    const refId = Math.random().toString(36).slice(2, 8).toUpperCase();
    return { failed: true, refId };
  }

  render() {
    if (this.state.failed) {
      return (
        <ErrorState
          title={m.design.errorTitle}
          message={fmt(m.design.errorMessage, { ref: this.state.refId })}
          retryLabel={m.design.reload}
          onRetry={() => window.location.reload()}
        />
      );
    }
    return this.props.children;
  }
}
