import { createContext, useContext } from 'react';

export type ToastRequest = {
  message: string;
  actionLabel?: string;
  onAction?: () => void;
};

export const ToastContext = createContext<(toast: ToastRequest) => void>(() => {});

export function useToast() {
  return useContext(ToastContext);
}
