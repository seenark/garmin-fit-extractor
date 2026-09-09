# Runner’s Garage

Runner’s Garage is the single deployed workspace for runners, with two clear
areas:

- `Runs`, the preserved Garmin FIT workflow for importing activity exports,
  inspecting normalized/raw JSON, and keeping private history.
- `Shoes`, a public running-shoe library with reviewer evidence, size charts,
  and conservative cross-shoe comparison.

The repository still contains `@garmin-fit-extractor/cli`, the preserved
`garmin-coach analyze` CLI. The TanStack Router React web UI and Axum +
PostgreSQL API serve both product areas; Google authentication protects Runs
and transcript administration, while the catalog remains static frontend data.

The public shoe catalog is checked-in frontend data and images. It has no database tables or API. The service stores only normalized/raw JSON, extraction metadata, and private intake records. Original ZIP/FIT bytes, temporary upload files, client paths, and transcription content are never copied into public/static data.

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

## Routes and API behavior

The Axum API listens on `GARMIN_FIT_BIND` (default `0.0.0.0:3000`) and exposes:

- `GET /api/v1/auth/login` and `GET /api/v1/auth/callback` for Google OAuth.
- `GET /api/v1/auth/me` and `POST /api/v1/auth/logout` for the current session.
- `POST /api/v1/extractions` with repeated multipart `files` fields containing ZIP archives.
- `GET /api/v1/extractions?limit=50&offset=0&order=desc` and `GET /api/v1/extractions/{id}`.
- `GET /api/v1/extractions/{id}/download?view=normalized|raw`.
- `DELETE /api/v1/extractions/{id}` and `DELETE /api/v1/extractions`.
- `GET /api/v1/activities/latest`, `/api/v1/activities`, and `/api/v1/activities/{id}` for FIT Coach OAuth clients.
- `GET|POST|DELETE /api/admin/transcript-entries` and `GET|PUT|DELETE /api/admin/transcript-entries/{id}` for allowlisted Garmin sessions. Collection deletion requires `{ "confirmation": "DELETE_ALL" }`.
- `GET /healthz` (public).

The web routes `/` and `/shoes` are public product entry points. `/upload`,
`/history`, and `/extractions/{id}` retain their Google-session behavior. The
Shoes routes `/shoes` and `/shoes/{shoeId}` are public static catalog routes.
`/admin/transcripts` is presented in the authenticated shell, but API
authorization remains authoritative. See [`docs/admin-transcripts.md`](docs/admin-transcripts.md)
for the complete admin workflow and API contract.

All extraction routes require a valid Google session. Uploads accept 1–10 archives. Each archive must have a case-insensitive `.zip` suffix and is limited to 20 MiB compressed. The request body limit is 210 MiB. Each archive may contain at most 50 FIT members, each at most 20 MiB uncompressed, with a 100 MiB total uncompressed FIT limit. Invalid names, oversized files, invalid archives, no-FIT archives, and FIT decode/CRC failures are persisted as independent failed rows; valid siblings still complete. Per-file errors include `INVALID_FILE_NAME`, `FILE_TOO_LARGE`, `INVALID_ZIP`, `ARCHIVE_LIMIT_EXCEEDED`, `NO_FIT_FILES`, or `INVALID_FIT`.

Successful rows retain compact normalized and raw JSON. Failed rows retain a stable error code/message and null JSON views. History is scoped to the signed-in user, ordered by activity date with undated rows last, and manually retained until deleted.

Transcript responses are never public. Admin requests require a valid Garmin session and an email normalized with trim plus ASCII lowercase to match `ADMIN_EMAILS`; a missing or empty allowlist denies every request. The allowlisted account receives the `Admin queue` navigation entry. Mutating admin transcript requests additionally require an exact `Origin` match with the configured `GARMIN_FIT_APP_ORIGIN`; an unset origin denies every mutation. Duplicate `video_id` values return `409 Conflict`. The admin form accepts channel name, YouTube URL, and transcription; the server derives and validates `video_id` from the URL.

## Configuration

Copy `.env.example` to `.env` and replace placeholders with deployment values. Never commit `.env` or real credentials.

| Variable | Default | Purpose |
| `POSTGRES_DB` | required | PostgreSQL database name |
| `POSTGRES_USER` | required | PostgreSQL role |
| `POSTGRES_PASSWORD` | required | PostgreSQL password |
| `DATABASE_URL` | required | PostgreSQL URL used by Axum |
| `ADMIN_EMAILS` | empty | Comma-separated verified Google emails; empty denies all admin access |
| `GARMIN_FIT_APP_ORIGIN` | unset | Browser-visible HTTP(S) origin required by admin transcript mutations; path/query/fragment values are invalid, and unset denies mutations |
| `GARMIN_FIT_IMAGE` | `hadesgod/garmin-fit-extractor` | Docker image repository used by Compose |
| `GARMIN_FIT_TAG` | `latest` | Docker image tag used by Compose |
| `GARMIN_FIT_PORT` | `8100` | Host port published by Compose |
| `GARMIN_FIT_BIND` | `0.0.0.0:3000` | Axum bind address |
| `GARMIN_FIT_STATIC_DIR` | `/app/public` | Built SPA directory |
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

Build the single production image (the runtime does not contain Bun, Cargo, source, fixtures, the TypeScript CLI, or SQLite):

```bash
docker buildx build --load -t hadesgod/garmin-fit-extractor:local .
cp .env.example .env
# Edit .env with generated PostgreSQL credentials and OAuth values.
GARMIN_FIT_TAG=local docker compose up -d
curl --fail http://localhost:8100/healthz
```

`compose.yaml` runs PostgreSQL 18 as `postgres` and the unified Axum/web service as `garmin-fit-extractor`. PostgreSQL uses the required `./db-data:/var/lib/postgresql` bind mount, publishes `5432`, and is joined to the `intranet` network. The app waits for the `pg_isready` healthcheck and publishes `${GARMIN_FIT_PORT:-8100}` to container port `3000`. Do not expose `5432` beyond the intended intranet; host firewall policy is a deployment gate.

Recreate both services without deleting `db-data` to verify persistence:

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

On Ubuntu, install Docker Engine and the Compose plugin, keep `.env` private, run `docker compose pull && docker compose up -d`, and set `GARMIN_FIT_GOOGLE_REDIRECT_URI` to the public API callback URL with TLS enabled.

## SQLite-to-PostgreSQL migration

The isolated `legacy-data-migrator` is dry-run by default. Use only stopped, copied SQLite sources or standalone snapshots; never point it at a live Docker volume. Follow [`docs/postgresql-migration.md`](docs/postgresql-migration.md) for snapshot, prepare-target, dry-run, apply, verify, rollback, and sensitive-artifact handling. Reports contain counts and checksums only.


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

## Backups and rollback

Keep source volume archives and standalone SQLite snapshots private and immutable until cutover acceptance. The unified service must not mount the old SQLite volume. Before traffic cutover, retain a schema-only dump, post-import PostgreSQL dump, count/checksum reports, and the original source archives with restricted permissions.

Once PostgreSQL accepts new application writes, do not roll back to the old SQLite services: this project has no reverse synchronizer. Remove unified traffic, preserve `db-data` and the PostgreSQL dump, and use a validated roll-forward repair or a separately validated reverse migration.

## FIT fixture attribution

`apps/api/tests/fixtures/activity.fit` and its ZIP archive are copied from fitparser's MIT-licensed `tests/fixtures/Activity.fit` fixture and are used only for decoder, API, E2E, and container tests.
