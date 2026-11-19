import { http, HttpResponse } from 'msw';
import { setupServer } from 'msw/node';
import type { Me, Snippet } from '../api/types';

/** Shared MSW server for the web test suites. Default: logged
 * out. Suites override handlers with server.use(...). */

export const meFixture: Me = {
  username: 'ada',
  email: 'ada@example.com',
  display_name: 'Ada',
  bio: null,
  created_at: '2026-01-01T00:00:00Z',
  csrf_token: 'test-csrf-token',
};

export function snippetFixture(overrides: Partial<Snippet> = {}): Snippet {
  return {
    owner: 'ada',
    name: 'ubuntu-setup',
    description: 'Update packages and install basics',
    visibility: 'private',
    tags: ['ubuntu'],
    version: 3,
    hash: 'a'.repeat(64),
    short_hash: 'aaaaaaaa',
    body: 'name: ubuntu-setup\n',
    message: null,
    star_count: 0,
    fork_count: 0,
    pull_count: 0,
    forked_from: null,
    starred_by_me: false,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-02T00:00:00Z',
    ...overrides,
  };
}

export const defaultHandlers = [
  http.get('/api/v1/me', () => HttpResponse.json({ error: { code: 'unauthorized', message: 'Login required' } }, { status: 401 })),
];

export const server = setupServer(...defaultHandlers);

export function authedHandlers() {
  return [
    http.get('/api/v1/me', () => HttpResponse.json(meFixture)),
    http.post('/api/v1/auth/logout', () => new HttpResponse(null, { status: 204 })),
  ];
}
