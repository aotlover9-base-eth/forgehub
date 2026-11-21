import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { api, apiRaw, ApiError, setCsrfToken } from './client';
import type {
  ApiTokenInfo,
  CreatedToken,
  Me,
  PublicProfile,
  SearchPage,
  SessionInfo,
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

/** Ask for a password-reset email. The server answers 204
 * whether or not the address is registered, so the UI shows
 * the same "check your inbox" state either way. */
export function usePasswordResetRequest() {
  return useMutation<void, ApiError, { email: string }>({
    mutationFn: (v) => api<void>('/auth/password-reset/request', { method: 'POST', body: JSON.stringify(v) }),
  });
}

/** Set a new password from a reset link's token. */
export function usePasswordResetConfirm() {
  return useMutation<void, ApiError, { token: string; new_password: string }>({
    mutationFn: (v) => api<void>('/auth/password-reset/confirm', { method: 'POST', body: JSON.stringify(v) }),
  });
}

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

export function useSnippet(owner: string, name: string, enabled = true) {
  return useQuery<Snippet>({
    queryKey: ['snippet', owner, name],
    enabled,
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

/** Forks of a snippet that the viewer may see (Peak A5). */
export function useForks(owner: string, name: string, enabled: boolean) {
  return useQuery<{ forks: Snippet[] }>({
    queryKey: ['forks', owner, name],
    enabled,
    retry: false,
    queryFn: () => api<{ forks: Snippet[] }>(`${snippetPath(owner, name)}/forks`),
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

// ---------------------------------------------------------------------------
// Explore, profiles, settings, device (Step 17)
// ---------------------------------------------------------------------------

export type SearchSort = 'relevance' | 'stars' | 'updated';

export function useSearch(query: string, tag: string | null, sort: SearchSort) {
  return useInfiniteQuery<SearchPage>({
    queryKey: ['search', query, tag, sort],
    enabled: query.trim().length > 0 || tag !== null,
    initialPageParam: null as string | null,
    getNextPageParam: (last) => last.next_cursor,
    queryFn: ({ pageParam }) => {
      const params = new URLSearchParams({ sort, limit: '20' });
      if (query.trim()) params.set('q', query.trim());
      if (tag) params.set('tag', tag);
      if (pageParam) params.set('cursor', pageParam as string);
      return api<SearchPage>(`/search?${params.toString()}`);
    },
  });
}

export function useExplore(feed: 'recent' | 'popular') {
  return useInfiniteQuery<SnippetPage>({
    queryKey: ['explore', feed],
    initialPageParam: null as string | null,
    getNextPageParam: (last) => last.next_cursor,
    queryFn: ({ pageParam }) => {
      const params = new URLSearchParams({ feed, limit: '20' });
      if (pageParam) params.set('cursor', pageParam as string);
      return api<SnippetPage>(`/explore?${params.toString()}`);
    },
  });
}

export function usePopularTags() {
  return useQuery<{ tags: { tag: string; count: number }[] }>({
    queryKey: ['popular-tags'],
    queryFn: () => api<{ tags: { tag: string; count: number }[] }>('/tags'),
  });
}

export function useProfile(username: string) {
  return useQuery<PublicProfile>({
    queryKey: ['profile', username],
    retry: false,
    queryFn: () => api<PublicProfile>(`/users/${encodeURIComponent(username)}`),
  });
}

export function useProfileSnippets(username: string) {
  return useQuery<SnippetPage>({
    queryKey: ['profile-snippets', username],
    retry: false,
    queryFn: () =>
      api<SnippetPage>(`/users/${encodeURIComponent(username)}/snippets?limit=100&sort=updated`),
  });
}

export function usePatchMe() {
  const queryClient = useQueryClient();
  return useMutation<Me, ApiError, { display_name?: string | null; bio?: string | null }>({
    mutationFn: (patch) => api<Me>('/me', { method: 'PATCH', body: JSON.stringify(patch) }),
    onSuccess: (me) => {
      if (me.csrf_token) setCsrfToken(me.csrf_token);
      queryClient.setQueryData(['me'], me);
    },
  });
}

export function useChangePassword() {
  const queryClient = useQueryClient();
  return useMutation<Me, ApiError, { current_password: string; new_password: string }>({
    mutationFn: (body) => api<Me>('/me', { method: 'PATCH', body: JSON.stringify(body) }),
    onSuccess: (me) => {
      // The password change rotated this session; the fresh
      // CSRF token comes back in the response.
      if (me.csrf_token) setCsrfToken(me.csrf_token);
      queryClient.setQueryData(['me'], me);
    },
  });
}

export function useSessions() {
  return useQuery<{ sessions: SessionInfo[] }>({
    queryKey: ['sessions'],
    retry: false,
    queryFn: () => api<{ sessions: SessionInfo[] }>('/me/sessions'),
  });
}

export function useRevokeSession() {
  const queryClient = useQueryClient();
  return useMutation<void, ApiError, string>({
    mutationFn: (ref) => api<void>(`/me/sessions/${encodeURIComponent(ref)}`, { method: 'DELETE' }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['sessions'] }),
  });
}

export function useLogoutAll() {
  const queryClient = useQueryClient();
  return useMutation<void, ApiError, void>({
    mutationFn: () => api<void>('/auth/logout-all', { method: 'POST' }),
    onSettled: () => {
      setCsrfToken(null);
      queryClient.setQueryData(['me'], null);
    },
  });
}

export function useDeleteAccount() {
  const queryClient = useQueryClient();
  return useMutation<void, ApiError, { username: string; password: string }>({
    mutationFn: (body) => api<void>('/me', { method: 'DELETE', body: JSON.stringify(body) }),
    onSettled: () => {
      setCsrfToken(null);
      queryClient.setQueryData(['me'], null);
    },
  });
}

export function useTokens() {
  return useQuery<{ tokens: ApiTokenInfo[] }>({
    queryKey: ['tokens'],
    retry: false,
    queryFn: () => api<{ tokens: ApiTokenInfo[] }>('/tokens'),
  });
}

export function useCreateToken() {
  const queryClient = useQueryClient();
  return useMutation<CreatedToken, ApiError, { name: string; scopes: string[]; expires_in_days?: number }>({
    mutationFn: (body) => api<CreatedToken>('/tokens', { method: 'POST', body: JSON.stringify(body) }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['tokens'] }),
  });
}

export function useRevokeToken() {
  const queryClient = useQueryClient();
  return useMutation<void, ApiError, number>({
    mutationFn: (id) => api<void>(`/tokens/${id}`, { method: 'DELETE' }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['tokens'] }),
  });
}

export function useDeviceLookup(userCode: string | null) {
  return useQuery<{ client_name: string; scopes: string[]; expires_at: string }>({
    queryKey: ['device-lookup', userCode],
    enabled: !!userCode,
    retry: false,
    queryFn: () =>
      api<{ client_name: string; scopes: string[]; expires_at: string }>(
        `/auth/device/lookup?user_code=${encodeURIComponent(userCode!)}`,
      ),
  });
}

export function useDeviceDecision() {
  return useMutation<void, ApiError, { user_code: string; approve: boolean }>({
    mutationFn: ({ user_code, approve }) =>
      api<void>(`/auth/device/${approve ? 'approve' : 'deny'}`, {
        method: 'POST',
        body: JSON.stringify({ user_code }),
      }),
  });
}
