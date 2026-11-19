import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api, apiRaw, ApiError, setCsrfToken } from './client';
import type {
  Me,
  Snippet,
  SnippetInput,
  SnippetPage,
  ValidationResult,
  VersionSummary,
  Visibility,
} from './types';

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

// ---------------------------------------------------------------------------
// Snippet detail, editor and social actions (Step 16)
// ---------------------------------------------------------------------------

const snippetPath = (owner: string, name: string) =>
  `/snippets/${encodeURIComponent(owner)}/${encodeURIComponent(name)}`;

export function useSnippet(owner: string, name: string) {
  return useQuery<Snippet>({
    queryKey: ['snippet', owner, name],
    retry: false,
    queryFn: () => api<Snippet>(snippetPath(owner, name)),
  });
}

export function useSnippetVersion(owner: string, name: string, version: number | null) {
  return useQuery<Snippet>({
    queryKey: ['snippet', owner, name, 'version', version],
    enabled: version !== null,
    retry: false,
    queryFn: () => api<Snippet>(`${snippetPath(owner, name)}/versions/${version}`),
  });
}

export function useVersions(owner: string, name: string, enabled: boolean) {
  return useQuery<{ versions: VersionSummary[] }>({
    queryKey: ['versions', owner, name],
    enabled,
    retry: false,
    queryFn: () => api<{ versions: VersionSummary[] }>(`${snippetPath(owner, name)}/versions`),
  });
}

export function useDiff(owner: string, name: string, from: number | null, to: number | null) {
  return useQuery<{ from: number; to: number; diff: string }>({
    queryKey: ['diff', owner, name, from, to],
    enabled: from !== null && to !== null,
    retry: false,
    queryFn: () =>
      api<{ from: number; to: number; diff: string }>(
        `${snippetPath(owner, name)}/diff?from=${from}&to=${to}`,
      ),
  });
}

/** Validate via the server endpoint; a 422 still carries the
 * result body (secret findings), so it resolves, not throws. */
export function useValidate() {
  return useMutation<ValidationResult, ApiError, SnippetInput>({
    mutationFn: async (input) => {
      const { status, body } = await apiRaw('/snippets/validate', {
        method: 'POST',
        body: JSON.stringify(input),
      });
      if (status === 200 || status === 422) return body as ValidationResult;
      const err = body as { error?: { code: string; message: string } } | null;
      throw new ApiError(status, err?.error?.code ?? 'http_error', err?.error?.message ?? 'Validation failed');
    },
  });
}

/** Name availability for the signed-in user: 'taken' when one
 * of their snippets already has the name, 'free' on 404. */
export function useNameAvailability(username: string | undefined, name: string, enabled: boolean) {
  return useQuery<'taken' | 'free'>({
    queryKey: ['name-availability', username, name],
    enabled: enabled && !!username && name.length >= 2,
    retry: false,
    queryFn: async () => {
      try {
        await api<Snippet>(snippetPath(username!, name));
        return 'taken';
      } catch (error) {
        if (error instanceof ApiError && error.status === 404) return 'free';
        throw error;
      }
    },
  });
}

export function useCreateSnippet() {
  const queryClient = useQueryClient();
  return useMutation<Snippet, ApiError, SnippetInput>({
    mutationFn: (input) => api<Snippet>('/snippets', { method: 'POST', body: JSON.stringify(input) }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['my-snippets'] }),
  });
}

export function useUpdateSnippet() {
  const queryClient = useQueryClient();
  return useMutation<
    Snippet,
    ApiError,
    { owner: string; name: string; ifMatch: number; input: SnippetInput }
  >({
    mutationFn: ({ owner, name, ifMatch, input }) =>
      api<Snippet>(snippetPath(owner, name), {
        method: 'PUT',
        headers: { 'If-Match': String(ifMatch) },
        body: JSON.stringify(input),
      }),
    onSuccess: (_data, vars) => {
      queryClient.invalidateQueries({ queryKey: ['snippet', vars.owner, vars.name] });
      queryClient.invalidateQueries({ queryKey: ['my-snippets'] });
    },
  });
}

export function usePatchSnippet() {
  const queryClient = useQueryClient();
  return useMutation<
    Snippet,
    ApiError,
    { owner: string; name: string; patch: { description?: string; visibility?: Visibility; tags?: string[] } }
  >({
    mutationFn: ({ owner, name, patch }) =>
      api<Snippet>(snippetPath(owner, name), { method: 'PATCH', body: JSON.stringify(patch) }),
    onSuccess: (_data, vars) => {
      queryClient.invalidateQueries({ queryKey: ['snippet', vars.owner, vars.name] });
      queryClient.invalidateQueries({ queryKey: ['my-snippets'] });
    },
  });
}

export function useStar() {
  const queryClient = useQueryClient();
  return useMutation<void, ApiError, { owner: string; name: string; starred: boolean }>({
    mutationFn: ({ owner, name, starred }) =>
      api<void>(`${snippetPath(owner, name)}/star`, { method: starred ? 'DELETE' : 'PUT' }),
    onSuccess: (_data, vars) =>
      queryClient.invalidateQueries({ queryKey: ['snippet', vars.owner, vars.name] }),
  });
}

export function useFork() {
  return useMutation<Snippet, ApiError, { owner: string; name: string; newName?: string }>({
    mutationFn: ({ owner, name, newName }) =>
      api<Snippet>(`${snippetPath(owner, name)}/fork`, {
        method: 'POST',
        body: JSON.stringify(newName ? { name: newName } : {}),
      }),
  });
}

export function useReport() {
  return useMutation<void, ApiError, { owner: string; name: string; reason: string }>({
    mutationFn: ({ owner, name, reason }) =>
      api<void>(`${snippetPath(owner, name)}/report`, {
        method: 'POST',
        body: JSON.stringify({ reason }),
      }),
  });
}
