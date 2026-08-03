# Garmin FIT Extractor

Garmin FIT Extractor is a Bun workspace containing:

- `@garmin-fit-extractor/cli`, the preserved `garmin-coach analyze` CLI.
- A TanStack Router React web UI for uploads, history, normalized analysis, and raw decoded JSON.
- An Axum + SQLite API that decodes batches of FIT files and serves the UI from the same origin.

The service stores only normalized/raw JSON and extraction metadata. Original FIT bytes, temporary upload files, and client paths are never retained.

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

The web development server proxies `/api` and `/healthz` to Axum at `127.0.0.1:3000`. Production is same-origin and does not configure CORS or application authentication. Put the unauthenticated service behind a trusted homelab network and an authenticated/TLS reverse proxy before exposing it publicly.

## Preserved CLI

```bash
bun run --filter @garmin-fit-extractor/cli build
bun run --filter @garmin-fit-extractor/cli test
bun run --filter @garmin-fit-extractor/cli garmin-coach analyze activity.fit --output runs/activity.json
```

The CLI writes two-space JSON ending in a newline, preserves `schemaVersion: "1.0.0"`, and prints the absolute output path. Its JavaScript FIT SDK normalization contract remains unchanged.

## API behavior

The Axum API listens on `GARMIN_FIT_BIND` (default `0.0.0.0:3000`) and exposes:

- `POST /api/v1/extractions` with repeated multipart `files` fields.
- `GET /api/v1/extractions?limit=50&offset=0` and `GET /api/v1/extractions/{id}`.
- `GET /api/v1/extractions/{id}/download?view=normalized|raw`.
- `DELETE /api/v1/extractions/{id}` and `DELETE /api/v1/extractions`.
- `GET /healthz`.

Uploads accept 1–10 files. Each file must have a case-insensitive `.fit` suffix and is limited to 20 MiB. The request body limit is 210 MiB, leaving multipart framing overhead for ten maximum-size files. Invalid names, oversized files, and CRC/decode failures are persisted as independent failed extraction rows; valid siblings still complete. Per-file errors are `INVALID_FILE_NAME`, `FILE_TOO_LARGE`, or `INVALID_FIT`.

Failed rows retain a stable error code/message and null JSON views. Successful rows retain compact normalized and raw JSON. History is manually retained until deleted; no automatic expiration exists.

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `GARMIN_FIT_BIND` | `0.0.0.0:3000` | Axum bind address |
| `GARMIN_FIT_DATABASE_URL` | `sqlite://data/garmin-fit-extractor.sqlite3` | SQLite database |
| `GARMIN_FIT_STATIC_DIR` | `apps/web/dist` | Built SPA directory |
| `RUST_LOG` | `info` | tracing filter |

The API creates the database parent directory, enables SQLite WAL mode, and uses a five-second busy timeout.

## Local Docker deployment

Build the single production image (the runtime does not contain Bun, Cargo, source, fixtures, or the TypeScript CLI):

```bash
docker buildx build --load -t hadesgod/garmin-fit-extractor:local .
GARMIN_FIT_TAG=local docker compose up -d
curl --fail http://localhost:3000/healthz
```

`compose.yaml` uses the image `hadesgod/garmin-fit-extractor:${GARMIN_FIT_TAG:-latest}`, publishes `${GARMIN_FIT_PORT:-3000}`, and persists SQLite in the stable Docker volume `garmin_fit_data`. Recreate without `-v` to verify persistence:

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

On Ubuntu, install Docker Engine and the Compose plugin, copy `compose.yaml`, create `.env` containing exactly `GARMIN_FIT_TAG=0.1.0` and `GARMIN_FIT_PORT=3000`, then run `docker compose pull && docker compose up -d`. Keep TLS and remote authentication at the existing reverse proxy.

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

`apps/api/tests/fixtures/activity.fit` is copied from fitparser's MIT-licensed `tests/fixtures/Activity.fit` fixture and is used only for decoder, API, E2E, and container tests.
