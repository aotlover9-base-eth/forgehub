import { fmt, m } from '../i18n/messages';
import type { ApiErrorBody } from './types';

/**
 * The typed API client. One place knows about: the base path,
 * the error envelope, the CSRF token (kept in memory, taken
 * from /me and auth responses), and turning failures into
 * calm UI messages.
 */

const BASE = '/api/v1';

let csrfToken: string | null = null;
export function setCsrfToken(token: string | null) {
  csrfToken = token;
}
export function getCsrfToken() {
  return csrfToken;
}

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly retryAfterSeconds: number | null;

  constructor(status: number, code: string, message: string, retryAfterSeconds: number | null = null) {
    super(message);
    this.status = status;
    this.code = code;
    this.retryAfterSeconds = retryAfterSeconds;
  }

  /** A calm, user-facing message for this failure. */
  get uiMessage(): string {
    if (this.status === 429) {
      return this.retryAfterSeconds
        ? fmt(m.api.rateLimited, { seconds: this.retryAfterSeconds })
        : m.api.rateLimitedShort;
    }
    if (this.status === 0) {
      return m.api.unreachable;
    }
    return this.message;
  }
}

function parseJson(text: string): unknown {
  if (!text) return null;
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function parseRetryAfter(response: Response): number | null {
  const raw = response.headers.get('Retry-After');
  if (!raw) return null;
  const seconds = Number.parseInt(raw, 10);
  return Number.isFinite(seconds) ? seconds : null;
}

export async function api<T>(path: string, init: RequestInit = {}): Promise<T> {
  const method = (init.method ?? 'GET').toUpperCase();
  const headers = new Headers(init.headers);
  if (init.body !== undefined && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json');
  }
  if (method !== 'GET' && csrfToken) {
    headers.set('X-CSRF-Token', csrfToken);
  }
  let response: Response;
  try {
    response = await fetch(`${BASE}${path}`, { ...init, headers });
  } catch {
    throw new ApiError(0, 'network', 'Network failure');
  }
  if (response.status === 204) {
    return undefined as T;
  }
  const text = await response.text();
  const parsed: unknown = parseJson(text);
  if (!response.ok) {
    const body = parsed as ApiErrorBody | null;
    const code = body?.error?.code ?? 'http_error';
    const message = body?.error?.message ?? `Request failed (${response.status})`;
    throw new ApiError(response.status, code, message, parseRetryAfter(response));
  }
  return parsed as T;
}

export function apiUrl(path: string): string {
  return `${BASE}${path}`;
}

/**
 * Raw variant for endpoints whose error responses carry a
 * useful body (snippet validation returns its result with a
 * 422 when secrets are found). Returns the status and the
 * parsed body; transport failures still throw ApiError(0).
 */
export async function apiRaw(
  path: string,
  init: RequestInit = {},
): Promise<{ status: number; body: unknown }> {
  const method = (init.method ?? 'GET').toUpperCase();
  const headers = new Headers(init.headers);
  if (init.body !== undefined && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json');
  }
  if (method !== 'GET' && csrfToken) {
    headers.set('X-CSRF-Token', csrfToken);
  }
  let response: Response;
  try {
    response = await fetch(`${BASE}${path}`, { ...init, headers });
  } catch {
    throw new ApiError(0, 'network', 'Network failure');
  }
  const text = await response.text();
  return { status: response.status, body: parseJson(text) };
}
