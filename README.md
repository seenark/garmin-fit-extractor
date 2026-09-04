# Garmin FIT Extractor

Garmin FIT Extractor is a Bun workspace containing:

- `@garmin-fit-extractor/cli`, the preserved `garmin-coach analyze` CLI.
- A TanStack Router React web UI for ZIP uploads, per-user history, normalized analysis, and raw decoded JSON.
- An Axum + SQLite API that authenticates users with Google and extracts FIT members from ZIP archives.

The service stores only normalized/raw JSON and extraction metadata. Original ZIP/FIT bytes, temporary upload files, and client paths are never retained.

## Requirements

- Bun 1.3.14
- Rust stable toolchain and Cargo
- Docker Engine with Buildx and Compose plugin for container verification
- At least 1 GiB memory for the production container; a ten-file, 20 MiB batch is intentionally bounded but decoding and JSON serialization add overhead.

## Workspace commands

```bash
bun install --frozen-lockfile
bun run dev       # Vite web server and Axum API
bun run check
bun run test
bun run build
bun run test:e2e
```

The web development server proxies `/api` and `/healthz` to Axum at `127.0.0.1:3000`. Production uses same-origin Google authentication and does not configure CORS. Set the Google OAuth variables before exposing the service publicly and use TLS so the callback and session cookie remain protected.

## Preserved CLI

```bash
bun run --filter @garmin-fit-extractor/cli build
bun run --filter @garmin-fit-extractor/cli test
bun run --filter @garmin-fit-extractor/cli garmin-coach analyze activity.fit --output runs/activity.json
```

The CLI writes two-space JSON ending in a newline, preserves `schemaVersion: "1.0.0"`, and prints the absolute output path. Its JavaScript FIT SDK normalization contract remains unchanged.

## API behavior

The Axum API listens on `GARMIN_FIT_BIND` (default `0.0.0.0:3000`) and exposes:

- `GET /api/v1/auth/login` and `GET /api/v1/auth/callback` for Google OAuth.
- `GET /api/v1/auth/me` and `POST /api/v1/auth/logout` for the current session.
- `POST /api/v1/extractions` with repeated multipart `files` fields containing ZIP archives.
- `GET /api/v1/extractions?limit=50&offset=0&order=desc` and `GET /api/v1/extractions/{id}`.
- `GET /api/v1/extractions/{id}/download?view=normalized|raw`.
- `DELETE /api/v1/extractions/{id}` and `DELETE /api/v1/extractions`.
- `GET /healthz` (public).

All extraction routes require a valid Google session. Uploads accept 1–10 archives. Each archive must have a case-insensitive `.zip` suffix and is limited to 20 MiB compressed. The request body limit is 210 MiB. Each archive may contain at most 50 FIT members, each at most 20 MiB uncompressed, with a 100 MiB total uncompressed FIT limit. Invalid names, oversized files, invalid archives, no-FIT archives, and FIT decode/CRC failures are persisted as independent failed rows; valid siblings still complete. Per-file errors include `INVALID_FILE_NAME`, `FILE_TOO_LARGE`, `INVALID_ZIP`, `ARCHIVE_LIMIT_EXCEEDED`, `NO_FIT_FILES`, or `INVALID_FIT`.

Successful rows retain compact normalized and raw JSON. Failed rows retain a stable error code/message and null JSON views. History is scoped to the signed-in user, ordered by activity date with undated rows last, and manually retained until deleted.

## Configuration

| Variable | Default | Purpose |
| `GARMIN_FIT_IMAGE` | `hadesgod/garmin-fit-extractor` | Docker image repository used by Compose |
| `GARMIN_FIT_TAG` | `latest` | Docker image tag used by Compose |
| `GARMIN_FIT_PORT` | `8100` | Host port published by Compose |
| `GARMIN_FIT_BIND` | `0.0.0.0:3000` | Axum bind address |
| `GARMIN_FIT_DATABASE_URL` | `sqlite://data/garmin-fit-extractor.sqlite3` | SQLite database |
| `GARMIN_FIT_STATIC_DIR` | `apps/web/dist` | Built SPA directory |
| `GARMIN_FIT_GOOGLE_CLIENT_ID` | unset | Google OAuth client ID |
| `GARMIN_FIT_GOOGLE_CLIENT_SECRET` | unset | Google OAuth client secret |
| `GARMIN_FIT_GOOGLE_REDIRECT_URI` | unset | Exact browser-visible OAuth callback URL |
| `GARMIN_FIT_CHATGPT_CLIENT_ID` | unset | FIT Coach OAuth client ID (`FIT_COACH_CHATGPT`) |
| `GARMIN_FIT_CHATGPT_CLIENT_SECRET` | unset | FIT Coach OAuth client secret |
| `GARMIN_FIT_CHATGPT_REDIRECT_URI` | unset | Exact OAuth callback URL shown by the GPT editor |
| `GARMIN_FIT_TEST_AUTH` | unset | Debug-only test login switch; ignored by release builds |
| `RUST_LOG` | `info` | tracing filter |

The three Google variables are all-or-none. If none are set, the service starts but Google login returns `AUTH_NOT_CONFIGURED`; a partial group is a configuration error. Sessions use a fixed 30-day lifetime. The development callback is `http://127.0.0.1:5173/api/v1/auth/callback` through the Vite proxy.

## Local Docker deployment

Build the single production image (the runtime does not contain Bun, Cargo, source, fixtures, or the TypeScript CLI):

```bash
docker buildx build --load -t hadesgod/garmin-fit-extractor:local .
GARMIN_FIT_TAG=local docker compose up -d
curl --fail http://localhost:8100/healthz
```

`compose.yaml` uses the fixed Compose project name `garmin-fit-extractor`, the image `hadesgod/garmin-fit-extractor:${GARMIN_FIT_TAG:-latest}`, publishes `${GARMIN_FIT_PORT:-8100}` on the host to the container's internal port `3000`, maps the Google OAuth and runtime variables into the container, and persists SQLite in the stable Docker volume `garmin_fit_data`. Pinning the project name prevents `docker compose up` from creating a sibling stack based on the directory name (for example `garmin-fit-extractor-repo`) and colliding on port `8100`. Recreate without `-v` to verify persistence:

```bash
docker compose down
GARMIN_FIT_TAG=local docker compose up -d
```

Build and publish both supported architectures after authenticating to Docker Hub:

```bash
docker login
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t hadesgod/garmin-fit-extractor:0.1.0 \
  -t hadesgod/garmin-fit-extractor:latest \
  --push .
```

On Ubuntu, install Docker Engine and the Compose plugin, copy `compose.yaml`, create `.env` containing the image/tag/port and Google variables, then run `docker compose pull && docker compose up -d`. Set `GARMIN_FIT_GOOGLE_REDIRECT_URI` to the public API callback URL and keep TLS enabled.

## FIT Coach

FIT Coach exposes owner-scoped activity data to a private Custom GPT through the handwritten contract in [`docs/fit-coach-openapi.yaml`](docs/fit-coach-openapi.yaml). The public hostname is one shared base: `https://fit.example.com/oauth/authorize`, `https://fit.example.com/oauth/token`, and the OpenAPI server all use `https://fit.example.com`. Keep the existing Google callback at `https://fit.example.com/api/v1/auth/callback`.

Set all three FIT Coach variables in the deployment environment (never commit secrets):

```dotenv
GARMIN_FIT_CHATGPT_CLIENT_ID=FIT_COACH_CHATGPT
GARMIN_FIT_CHATGPT_CLIENT_SECRET=
GARMIN_FIT_CHATGPT_REDIRECT_URI=
```

The GPT editor sequence is **Configure -> Actions -> Create new action -> Authentication -> OAuth**. Configure Client ID `FIT_COACH_CHATGPT`, the client secret, authorization URL `https://fit.example.com/oauth/authorize`, token URL `https://fit.example.com/oauth/token`, scope `activities:read`, and request-body token exchange. Copy the callback URL displayed by the editor exactly into `GARMIN_FIT_CHATGPT_REDIRECT_URI` and its server allowlist. Import `docs/fit-coach-openapi.yaml` after setting these values.

The Cloudflare route should map the public hostname to the application tunnel, which forwards to app port `3000`. Application routing handles `/oauth/*`, `/api/v1/*`, and the SPA/upload surface. Cloudflare Service Tokens are not user identity; FIT Coach identity comes only from OAuth and the authenticated browser session. Share the GPT by private link only with the two intended ChatGPT accounts, and verify activity access after connecting.

No MCP server, Apps SDK, Plugin/App Directory publication, or OpenAI API is required. Keep client secrets and callback-specific deployment values out of committed files.

คู่มือภาษาไทย:

- [ทดสอบ FIT Coach ในเครื่อง](docs/fit-coach-local-testing-th.md)
- [Deploy เว็บแบบ production-like โดยยังไม่เชื่อม Custom GPT](docs/deploy-manual-th.md)

## SQLite backup and restore

Stop Axum before touching the database. The volume name is stable across Compose project names:

```bash
mkdir -p backups
docker compose stop app
docker run --rm -v garmin_fit_data:/data:ro -v "$PWD/backups:/backup" alpine:3.22 sh -c 'tar -C /data -czf /backup/garmin-fit-data.tgz .'
docker compose start app
```

Restore only from a trusted archive while the app is stopped, then restore ownership for UID/GID `10001:10001`:

```bash
docker compose stop app
docker run --rm -v garmin_fit_data:/data -v "$PWD/backups:/backup:ro" alpine:3.22 sh -c 'rm -f /data/garmin-fit-extractor.sqlite3 /data/garmin-fit-extractor.sqlite3-wal /data/garmin-fit-extractor.sqlite3-shm && tar -C /data -xzf /backup/garmin-fit-data.tgz && chown -R 10001:10001 /data'
docker compose start app
```

Verify `/healthz` and a known history ID after restore.

## FIT fixture attribution

`apps/api/tests/fixtures/activity.fit` and its ZIP archive are copied from fitparser's MIT-licensed `tests/fixtures/Activity.fit` fixture and are used only for decoder, API, E2E, and container tests.
