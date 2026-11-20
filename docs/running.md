# Running ForgeHub

## Configuration

The server is configured entirely through environment variables
(see `.env.example`). Nothing secret is ever printed at startup -
the startup log shows the secret key only as set/unset + length.

| Variable | Default | Meaning |
|---|---|---|
| `FORGEHUB_ENV` | `dev` | `dev` (pretty logs) or `prod` (JSON logs; secret required) |
| `FORGEHUB_BIND` | `127.0.0.1:8080` | Listen address |
| `FORGEHUB_BASE_URL` | `http://127.0.0.1:8080` | Public base URL (password-reset links, device flow) |
| `FORGEHUB_SMTP_URL` | - | SMTP submission URL for outgoing mail, e.g. `smtps://user:pass@smtp.example.com:465` (carries a password - never logged) |
| `FORGEHUB_MAIL_FROM` | `ForgeHub <no-reply@localhost>` | From address for outgoing mail |
| `FORGEHUB_DATABASE_URL` | `sqlite://forgehub.db` | SQLite URL or path; `sqlite::memory:` works |
| `FORGEHUB_SECRET_KEY` | - | ≥ 32 bytes; **required in prod**, warned-about in dev |
| `FORGEHUB_ALLOW_SIGNUP` | `true` | Set `false` to close public registration |
| `FORGEHUB_WEB_DIR` | `web/dist` | Built web app; API-only mode with a warning when absent |
| `FORGEHUB_BODY_LIMIT_KIB` | `256` | Max API request body |
| `FORGEHUB_RATE_LIMIT_PER_MINUTE` | `600` | Per-token (else per-IP) request budget |
| `FORGEHUB_REQUEST_TIMEOUT_SECS` | `30` | API request timeout |
| `FORGEHUB_SESSION_IDLE_SECS` | `604800` | Session idle expiry (7 days) |
| `FORGEHUB_SESSION_ABSOLUTE_SECS` | `2592000` | Session absolute expiry (30 days) |
| `FORGEHUB_LOGIN_BACKOFF_BASE_MS` | `1000` | Login-throttle backoff base (doubles per failure, cap 300 s) |
| `FORGEHUB_DEVICE_POLL_INTERVAL_SECS` | `5` | Minimum seconds between device-code polls |
| `FORGEHUB_MAX_SNIPPETS_PER_USER` | `500` | Per-user snippet cap |
| `FORGEHUB_MAX_VERSIONS_PER_SNIPPET` | `200` | Per-snippet version cap |
| `RUST_LOG` | `forge_server=info` | Log filter (tracing EnvFilter syntax) |

## Development

```sh
# Terminal 1 - API server (migrations run automatically at start)
cargo run -p forge-server

# Terminal 2 - web app with hot reload; Vite proxies /api to :8080
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
public deployments. Shut down with SIGINT/SIGTERM - connections
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

## Admin commands

There is no admin web UI in v1. Moderation runs through the
server binary against its own database (same env config):

```bash
forge-server admin list-reports [--status open]
forge-server admin hide-snippet <owner>/<name>   # 404 everywhere public; owner keeps access
forge-server admin unhide-snippet <owner>/<name>
forge-server admin ban-user <username>           # disables + revokes sessions and tokens
forge-server admin unban-user <username>
```

## Notes

- **Backups:** the whole deployment state is the SQLite file -
  copy it (WAL checkpoint first: `sqlite3 forgehub.db .backup`).
- **Rate limiting** is in-memory and per-process; a multi-node
  deployment needs a shared limiter (recorded limitation).
- **CORS** is off by default: the web app is served same-origin.
  A separately hosted frontend is not a supported topology.
