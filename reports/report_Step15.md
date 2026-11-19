# Step 15 Report — Web auth screens and dashboard

## 1. Summary
The first real user flows: a typed API client with CSRF handling and calm error mapping, login/signup screens whose live hints mirror the server rules exactly, a logged-out landing page, and the dashboard ("Your snippets") with filters, sort, per-item actions, and the empty state with the install-the-CLI card.

## 2. What was built
- **API layer** (`web/src/api/`): `types.ts` hand-written from `docs/openapi.yaml` (no offline generator available; the file says so and is kept in sync manually); `client.ts` — one fetch wrapper owning the base path, the error envelope, the in-memory CSRF token (taken from `/me` and auth responses, sent as `X-CSRF-Token` on mutations), `Retry-After` parsing, and `ApiError.uiMessage` (429 → "wait N seconds", network → offline message); `hooks.ts` — TanStack Query hooks (`useMe` with 401 → null, login/signup/logout, `useMySnippets(filter, sort)`, delete, set-visibility) with cache invalidation.
- **Validation mirrors** (`web/src/validation.ts`): username rules identical to the server (length, charset, hyphen rules, and the reserved-word list copied from forge-core `names.rs` with a keep-in-sync comment), password ≥10/≤256/≠username, email shape, relative-time formatting.
- **Screens**: `/` landing (calm hero, the `fg ubuntu-setup` example in a CodeBlock, Create account + Explore; logged-in users redirect to the dashboard); `/login` (generic failure message — no account oracle — 429 shows the wait, show/hide password, `?return=` honoured); `/signup` (live hints while typing, `username_taken` mapped to the field, the generic 409 explained without confirming whether the email exists); `/dashboard` behind a `RequireAuth` gate (401 → login with return URL; a mid-visit 401 from the snippets query also redirects).
- **Dashboard**: search (client-side over the fetched page), visibility filter chips (All/Private/Unlisted/Public — the choice is passed to the API), sort menu (updated/created/name), list rows with mode icon, description, version, relative updated time, neutral tag chips, the Public chip as the only tertiary element; per-item menu: copy run command (`fg <name>`), copy raw URL, make private/unlisted/public, delete behind a danger `ConfirmDialog`; copy feedback via snackbar. Empty state: explanation + single "New snippet" action + install-the-CLI card (honest copy: installer packages arrive with the first release; README for now).
- **Keyboard**: `n` new snippet, `/` focuses the dashboard search, `?` opens a shortcuts help dialog; all ignored while typing in a field.
- The shell now shows the signed-in username + Log out (or Log in) in the app bar.

## 3. Tests and gates (real output)
- **MSW-mocked integration tests** (`src/pages/pages.test.tsx`, 12 tests): login success → return-URL navigation; 401 → generic message; 429 → "42 seconds"; signup live hints (charset + reserved word); `username_taken` → field error; dashboard list rendering with chips; empty state + install card; the visibility filter reaching the API as a query param; **session-expiry 401 → redirect to login**; axe-core with **zero violations** on login, signup and the populated dashboard.
- Two real test-infrastructure bugs found and fixed at the root: Testing Library auto-cleanup does not hook in with `globals: false` (renders leaked across tests — explicit `cleanup()` in `test-setup.ts`), and waitFor timeouts raised to 4 s for the shared-CPU parallel suite.
- Web gates: `npm run lint` (eslint + token check) clean; `typecheck` clean; `npm test` **25 passed (4 files)**; `npm run build` built.
- **Live smoke against the real server** (release binary serving `web/dist`): SPA index 200; `POST /auth/signup` → 201-style Me JSON with `csrf_token`; `POST /snippets` with the CSRF header → the created snippet; `GET /me/snippets?sort=updated` → the snippet listed. The full browser click-through at 360/768/1280 px in light/dark could not be captured in this environment (no browser); the adaptive behaviour is covered by the Step 14 width-class tests and the shell's three layouts, and the pages use only those layouts — recorded honestly as the QA method used here.

## 4. Deviations and problems
- MSW added as a devDependency for the mocked-API tests.
- The dashboard search filters the fetched page client-side (the `/me/snippets` endpoint has no text query; server search is for public discovery in Step 17). The list request caps at 100 items, matching the CLI's known page-size note.

## 5. Known gaps
- `/new` is still the placeholder (the editor is Step 16); `/explore` and `/profile` land in Step 17.
- No screenshots in `docs/screenshots/` — none could be captured reliably in this environment, per the step's own condition.

## 6. Next step
Step 16 — web snippet editor and detail pages.
