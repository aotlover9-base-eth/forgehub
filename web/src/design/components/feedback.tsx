import { useCallback, useRef, useState, type ReactNode } from 'react';
import { Button } from './core';
import { ToastContext, type ToastRequest } from './toast';

// ---------------------------------------------------------------------------
// Toasts (snackbar): one at a time, inverse-surface colours,
// role=status so assistive tech announces them.
// ---------------------------------------------------------------------------

type ToastState = ToastRequest & { id: number };

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toast, setToast] = useState<ToastState | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const show = useCallback((next: ToastRequest) => {
    window.clearTimeout(timer.current);
    setToast({ ...next, id: Date.now() });
    timer.current = window.setTimeout(() => setToast(null), 5000);
  }, []);
  return (
    <ToastContext.Provider value={show}>
      {children}
      {toast ? (
        <div className="fh-toast" role="status" key={toast.id}>
          <span className="fh-toast__message">{toast.message}</span>
          {toast.actionLabel ? (
            <button
              className="fh-toast__action"
              onClick={() => {
                toast.onAction?.();
                setToast(null);
              }}
            >
              {toast.actionLabel}
            </button>
          ) : null}
          <button className="fh-toast__close" aria-label="Dismiss" onClick={() => setToast(null)}>
            ×
          </button>
        </div>
      ) : null}
    </ToastContext.Provider>
  );
}

// ---------------------------------------------------------------------------
// Confirmation dialog (md-dialog). `danger` styles the confirm
// action with the error colour — reserved for destructive
// confirmations.
// ---------------------------------------------------------------------------

export function ConfirmDialog({
  open,
  headline,
  content,
  confirmLabel,
  cancelLabel = 'Cancel',
  danger = false,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  headline: string;
  content: ReactNode;
  confirmLabel: string;
  cancelLabel?: string;
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <md-dialog
      open={open || undefined}
      onClose={onCancel}
      onCancel={(e: Event) => {
        e.preventDefault();
        onCancel();
      }}
    >
      <div slot="headline">{headline}</div>
      <div slot="content">{content}</div>
      <div slot="actions">
        <Button variant="text" onClick={onCancel}>
          {cancelLabel}
        </Button>
        <span className={danger ? 'fh-danger-action' : undefined}>
          <Button variant="text" onClick={onConfirm}>
            {confirmLabel}
          </Button>
        </span>
      </div>
    </md-dialog>
  );
}
