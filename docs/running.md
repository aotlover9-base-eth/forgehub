# Running ForgeHub

## Configuration

The server is configured entirely through environment variables
(see `.env.example`). Nothing secret is ever printed at startup —
the startup log shows the secret key only as set/unset + length.

| Variable | Default | Meaning |
|---|---|---|
| `FORGEHUB_ENV` | `dev` | `dev` (pretty logs) or `prod` (JSON logs; secret required) |
| `FORGEHUB_BIND` | `127.0.0.1:8080` | Listen address |
| `FORGEHUB_BASE_URL` | `http://127.0.0.1:8080` | Public base URL (verification links, device flow) |
| `FORGEHUB_DATABASE_URL` | `sqlite://forgehub.db` | SQLite URL or path; `sqlite::memory:` works |
| `FORGEHUB_SECRET_KEY` | — | ≥ 32 bytes; **required in prod**, warned-about in dev |
| `FORGEHUB_ALLOW_SIGNUP` | `true` | Set `false` to close public registration |
| `FORGEHUB_WEB_DIR` | `web/dist` | Built web app; API-only mode with a warning when absent |
| `FORGEHUB_BODY_LIMIT_KIB` | `256` | Max API request body |
| `FORGEHUB_RATE_LIMIT_PER_MINUTE` | `600` | Per-token (else per-IP) request budget |
| `FORGEHUB_REQUEST_TIMEOUT_SECS` | `30` | API request timeout |
| `RUST_LOG` | `forge_server=info` | Log filter (tracing EnvFilter syntax) |

## Development

```sh
# Terminal 1 — API server (migrations run automatically at start)
cargo run -p forge-server

# Terminal 2 — web app with hot reload; Vite proxies /api to :8080
cd web && npm install && npm run dev
```

Check it:

```sh
curl localhost:8080/api/v1/health   # {"status":"ok","db":"ok"}
curl localhost:8080/api/v1/version  # {"version":"0.1.0","git_sha":"dev"}
```

## Production (single binary + static web)

```sh
cd web && npm ci && npm run build && cd ..
cargo build --release -p forge-server
FORGEHUB_ENV=prod \
FORGEHUB_SECRET_KEY="$(head -c 48 /dev/urandom | base64)" \
FORGEHUB_DATABASE_URL=sqlite:///var/lib/forgehub/forgehub.db \
FORGEHUB_WEB_DIR=web/dist \
./target/release/forge-server
```

The server serves the API under `/api/v1` and the web app from
`FORGEHUB_WEB_DIR` (hashed assets are cached immutably; `index.html`
is `no-cache`). Put a TLS-terminating reverse proxy in front for
public deployments. Shut down with SIGINT/SIGTERM — connections
drain gracefully.

## Docker

```sh
docker build -t forgehub .
docker run --rm -p 8080:8080 \
  -e FORGEHUB_SECRET_KEY="$(head -c 48 /dev/urandom | base64)" \
  -v forgehub-data:/data forgehub
```

The image runs the server as a non-root user with the database in
`/data` and `FORGEHUB_ENV=prod`.

## Notes

- **Backups:** the whole deployment state is the SQLite file —
  copy it (WAL checkpoint first: `sqlite3 forgehub.db .backup`).
- **Rate limiting** is in-memory and per-process; a multi-node
  deployment needs a shared limiter (recorded limitation).
- **CORS** is off by default: the web app is served same-origin.
  A separately hosted frontend is not a supported topology.
