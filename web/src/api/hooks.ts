import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api, ApiError, setCsrfToken } from './client';
import type { Me, SnippetPage, Visibility } from './types';

export type SnippetSort = 'updated' | 'created' | 'name';
export type VisibilityFilter = 'all' | Visibility;

/** The current session's user; null when logged out (401). */
export function useMe() {
  return useQuery<Me | null>({
    queryKey: ['me'],
    retry: false,
    queryFn: async () => {
      try {
        const me = await api<Me>('/me');
        if (me.csrf_token) setCsrfToken(me.csrf_token);
        return me;
      } catch (error) {
        if (error instanceof ApiError && error.status === 401) return null;
        throw error;
      }
    },
  });
}

function authMutation<TVars>(path: string, body: (vars: TVars) => unknown) {
  return function useAuthMutation() {
    const queryClient = useQueryClient();
    return useMutation<Me, ApiError, TVars>({
      mutationFn: (vars) => api<Me>(path, { method: 'POST', body: JSON.stringify(body(vars)) }),
      onSuccess: (me) => {
        if (me.csrf_token) setCsrfToken(me.csrf_token);
        queryClient.setQueryData(['me'], me);
      },
    });
  };
}

export const useLogin = authMutation<{ identifier: string; password: string }>('/auth/login', (v) => v);
export const useSignup = authMutation<{ username: string; email: string; password: string }>(
  '/auth/signup',
  (v) => v,
);

export function useLogout() {
  const queryClient = useQueryClient();
  return useMutation<void, ApiError, void>({
    mutationFn: () => api<void>('/auth/logout', { method: 'POST' }),
    onSettled: () => {
      setCsrfToken(null);
      queryClient.setQueryData(['me'], null);
      queryClient.removeQueries({ queryKey: ['my-snippets'] });
    },
  });
}

export function useMySnippets(filter: VisibilityFilter, sort: SnippetSort) {
  return useQuery<SnippetPage>({
    queryKey: ['my-snippets', filter, sort],
    queryFn: () => {
      const params = new URLSearchParams({ sort, limit: '100' });
      if (filter !== 'all') params.set('visibility', filter);
      return api<SnippetPage>(`/me/snippets?${params.toString()}`);
    },
  });
}

export function useDeleteSnippet() {
  const queryClient = useQueryClient();
  return useMutation<void, ApiError, { owner: string; name: string }>({
    mutationFn: ({ owner, name }) =>
      api<void>(`/snippets/${encodeURIComponent(owner)}/${encodeURIComponent(name)}`, {
        method: 'DELETE',
      }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['my-snippets'] }),
  });
}

export function useSetVisibility() {
  const queryClient = useQueryClient();
  return useMutation<void, ApiError, { owner: string; name: string; visibility: Visibility }>({
    mutationFn: ({ owner, name, visibility }) =>
      api<void>(`/snippets/${encodeURIComponent(owner)}/${encodeURIComponent(name)}`, {
        method: 'PATCH',
        body: JSON.stringify({ visibility }),
      }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['my-snippets'] }),
  });
}
