# Runner’s Garage

Runner’s Garage is the single workspace for runners, with two clear
areas:

- `Runs`, the Garmin FIT workflow for importing immutable activity sources,
  inspecting complete normalized/raw evidence, and keeping private history.
- `Shoes`, a public running-shoe library with reviewer evidence, size charts,
  and conservative cross-shoe comparison.

The repository still contains `@garmin-fit-extractor/cli`, the preserved
`garmin-coach analyze` CLI. The TanStack Router React web UI and Axum +
PostgreSQL API serve both product areas; Google authentication protects Runs
and transcript administration, while the catalog remains static frontend data.

The public shoe catalog is checked-in frontend data and images. It has no database tables or API. Runs retains immutable original FIT bytes and decoded/normalized/analysis revisions in owner-scoped PostgreSQL storage. Original ZIP/FIT bytes, temporary upload files, client paths, and transcription content are never copied into public/static data. Share-card photos remain local to the browser.

## Requirements

- Bun 1.3.14
- Rust stable toolchain and Cargo
- Docker Engine with Buildx and Compose plugin for container verification
- At least 1 GiB memory for the production container; a ten-file, 20 MiB batch is intentionally bounded but decoding and JSON serialization add overhead.

## Workspace commands

```bash
bun install --frozen-lockfile
bun run dev       # Vite web server and Axum API
bun --no-env-file run check
bun --no-env-file run test
bun --no-env-file run build
bun --no-env-file run test:e2e
```

Tests require an explicitly owned disposable PostgreSQL 18 `_test` or
`_rehearsal` database. The Rust consumer tests truncate authentication and
application tables; never use production, shared data, or an ordinary existing
development database. Set `TEST_DATABASE_URL` and `DATABASE_URL` to that target,
and set absolute `PGDATA` to the actual server data directory. Both test modes
verify the target identity before building or starting the API. `test:e2e`
never falls back to an ambient `DATABASE_URL`. Run from a secret-free source
archive with the outer `bun --no-env-file`; do not load private `.env` files.

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
- `POST /api/v2/runs/imports` with repeated multipart `files` containing FIT or ZIP.
- `GET /api/v2/runs?limit=50&offset=0&sort=startTime&order=desc`.
- `GET` and `DELETE /api/v2/runs/{id}` for coherent detail and permanent erasure.
- `POST /api/v2/runs/{id}/reprocess` to queue the existing immutable source.
- `GET /api/v2/runs/{id}/evidence` for event-time historical numerical evidence.
- `GET /api/v2/runs/thresholds/latest` and `/api/v2/runs/thresholds/trend`.
- `POST /api/v2/runs/exports` with an explicit Coach/Full selection and independent privacy consents.
- `GET` and `HEAD /api/v2/runs/exports/{token}` for owner-scoped pinned transport.
- `GET /api/v1/activities/latest`, `/api/v1/activities`, and `/api/v1/activities/{id}` for FIT Coach OAuth clients.
- `GET|POST|DELETE /api/admin/transcript-entries` and `GET|PUT|DELETE /api/admin/transcript-entries/{id}` for allowlisted Garmin sessions. Collection deletion requires `{ "confirmation": "DELETE_ALL" }`.
- `GET /healthz` (public).

The web routes `/` and `/shoes` are public product entry points. `/upload`,
`/history`, and `/extractions/{id}` retain their Google-session behavior. The
Shoes routes `/shoes` and `/shoes/{shoeId}` are public static catalog routes.
`/admin/transcripts` is presented in the authenticated shell, but API
authorization remains authoritative. See [`docs/admin-transcripts.md`](docs/admin-transcripts.md)
for the complete admin workflow and API contract.

Runs accepts direct FIT and ZIP content with bounded archive inspection and
ordered per-item outcomes. Unsupported or failed members do not erase valid
siblings. Identical bytes deduplicate within one owner; another owner remains
isolated. Source-less legacy activities retain safe summaries and explicit
`LEGACY_SOURCE_UNAVAILABLE`, without fabricated source bytes or numerical data.

Detail and exports preserve native values, full sample/RR resolution, source
references and missingness. Coach declares aggregation; Full retains permitted
native evidence. Export defaults omit location, device identity and unknown
opaque/developer data. Each consent is independent; non-null developer identity
is never promoted by consent. Copy reads exact pinned server text; Download uses
native browser transport. Snapshots expire after 15 minutes. Permanent deletion
revokes dependent snapshots and erases source and cached evidence.

The old browser `/api/v1/extractions*` upload/list/detail/export/delete routes
are retired: authenticated callers receive `410 RUNS_ENDPOINT_RETIRED`, not an
unredacted compatibility fallback. FIT Coach's OAuth `/api/v1/activities*`
projection and CLI schema `1.0.0` remain separate and preserved. See the
[Runs runtime contract](docs/runs-rebuild/runtime-contract.md) for limits,
experimental LT1/LT2 methods, exercised software evidence and scientific gates.

หลักฐาน integration รับรองเฉพาะ version และ scope ที่ตรวจ ไม่ใช่ release acceptance:
unchanged check/test/build ล่าสุดผ่าน 643.58 s: Bun 109, CLI TAP 8 และ Rust 221;
unchanged e2e ผ่าน 90.36 s โดย failed tests เป็น `[]` ไม่อนุมานจำนวน tests จาก buffered report
Current protected same-PG/API restart และ R5/R9 ทั้งเจ็ด gates รวม encrypted restore/native
re-decode ผ่านแล้ว Current isolated Linux faults ผ่าน; natural/worker/decode admission proofs
คง identity ก่อน builder-cancel fix ไม่ย้ายผลไปรับรอง ELF ใหม่ Browser 51 cases/privacy/selection
และ native Full 809,192,701 bytes/100k independent proof เป็น accepted pre-cancel evidence;
Current-source Full API runtime ผ่าน 141.81 s พร้อม frozen native100k oracle และ parent-memory
observations; fresh pin 809,192,713 bytes เป็นคนละ artifact กับ accepted browser pin เดิม
AT-34 exact-owned clone ผ่าน 27 cases/1235.26 s พร้อม independent review correct0.97/findings[]
Independent current-Full review correct0.96/findings[] ยืนยัน actual proof ตาม scope
สถานะและ final Git/workspace integration receipts ติดตามใน external evidence index;
selected-file receipts ไม่ใช่ whole-tree หรือ Docker-context identity

The grouping fix excludes only summary provenance source references from
observation identity: identical native streams with different FIT headers group
without changing preserved native references. The implemented `exact-stream-v2`
to `exact-stream-v3` observation-rule bump invalidates already stored caches
even though this integration is unreleased. Actual native reprocessing preserves
old revisions, source bytes, native references, sessions and pinned exports.
It does not change the scientific method, profile, schema or projection.

Final reviews found expired capacity leases could admit replacement while
physical CPU/decoder work remained alive. The implemented kernel-backed
physical-ownership fix and new-export/list/detail rule gates now have passing
scoped regressions; independent source reviews report no findings, not runtime
acceptance. The migration-directory tracking fix now passes scoped native
legacy verification and the unchanged full normal commands. The earlier
formatting/migration failures remain recorded, not hidden. หลักฐาน browser ก่อน builder-cancel
เก็บ debug timeout/stale state และ native reprocess ที่สำเร็จไว้โดยไม่เปลี่ยน caps/source bytes
Combined current-source review ไม่พบ findings แต่ไม่แทน AT-34 หรือ current Full runtime
ดู runtime contract สำหรับ exact identities/scopes และ scientific reference gates ที่ยังแยกอยู่

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

The image also contains the standalone operator-only `runs-reset`. It plans
read-only by default and requires explicit target/scope fingerprints, verified
backup and apply confirmations. Startup, migrations and deployment never run a
Runs reset. Use only the documented disposable-clone acceptance procedure.

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

Keep source volume archives and standalone SQLite snapshots private and immutable until cutover acceptance. The unified service must not mount the old SQLite volume. PostgreSQL backups must include original FIT `BYTEA`, immutable revisions/chunks, coherent manifests, durable jobs and protected auth/admin data. Before traffic cutover, retain restricted source archives, dumps and count/hash receipts. Authenticate encrypted backups before decrypting, restore only to a verified fresh disposable target, and prove exact original bytes, revision integrity, protected records and real native re-decoding. A preserved volume or CLI-only reset test does not establish backup/restore/reset acceptance.

Once PostgreSQL accepts new application writes, do not roll back to the old SQLite services: this project has no reverse synchronizer. Remove unified traffic, preserve `db-data` and the PostgreSQL dump, and use a validated roll-forward repair or a separately validated reverse migration.

## FIT fixture attribution

`apps/api/tests/fixtures/activity.fit` and its ZIP archive are copied from fitparser's MIT-licensed `tests/fixtures/Activity.fit` fixture and are used only for decoder, API, E2E, and container tests.
