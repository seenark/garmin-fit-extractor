# Runner’s Garage

Runner’s Garage is a workspace for runners, with two clear areas:

- `Runs`, the Garmin-running FIT workflow for importing sources, inspecting
  activity data and experimental analysis, and exporting selected private JSON
  for a manual AI handoff.
- `Shoes`, a public running-shoe library with reviewer evidence, size charts,
  and conservative cross-shoe comparison.

The repository still contains `@garmin-fit-extractor/cli`, the preserved
`garmin-coach analyze` CLI. The TanStack Router React web UI and Axum +
PostgreSQL API serve both product areas; Google authentication protects Runs
and transcript administration, while the catalog remains static frontend data.

The public shoe catalog is checked-in frontend data and images. It has no database tables or API. The Runs cutover separates immutable original FIT bytes, decoded archives, normalized data, analysis revisions, durable jobs, and short-lived exports in private PostgreSQL storage. Original ZIP archives are not retained. FIT bytes, client paths, exports, and transcription content never belong in public/static data.

**Integration status:** the decoder, analysis, and web changes are merged into
the integration branch. Core persistence/API changes are implemented on their
branch but have not yet been merged here. The Runs sections below describe the
frozen cutover contract, not an already verified deployment. Final integrated
runtime, browser, recovery, and full-suite checks remain outstanding. No
production deployment, Runs reset, or legacy migration apply is claimed.

## Requirements

- Bun 1.3.14
- Rust stable toolchain and Cargo
- Docker Engine with Buildx and Compose plugin for container verification
- PostgreSQL 18 and matching client tools for local verification
- A runnable native `garmin-fit-extractor-api` executable, including for tests
  that exercise its isolated decoder child
- Writable private temporary storage for decoder/export workspaces

Configured resource ceilings are not production performance guarantees.
Target-host memory, throughput, storage growth, and supported physical mobile
devices still require measurement.

## Workspace commands

```bash
bun --no-env-file install --frozen-lockfile
bun --no-env-file run dev       # Vite web server and Axum API
bun --no-env-file run check
bun --no-env-file run test
bun --no-env-file run build
bun --no-env-file run test:e2e
```

Tests require a newly created disposable **PostgreSQL 18** database. Rust
consumer tests truncate authentication and application tables; never use a
shared, production, or existing development database. Run from a secret-free
source archive with a clean explicit environment. The outer
`bun --no-env-file` matters: a nested flag cannot prevent the parent Bun
process from loading private `.env` files first.

Before either root `test` or `test:e2e`:

1. Create a uniquely owned disposable cluster/database and choose free loopback
   ports. Do not use normal Compose's persistent `./db-data` or stop another
   listener. Use PostgreSQL 18 server and matching client tools.
2. Set `TEST_DATABASE_URL` to an explicit `postgres://` or `postgresql://` URL
   containing host, user, and database. Set `DATABASE_URL` to the same URL.
   Neither command falls back to an ambient application database.
3. Set absolute `PGDATA` to the exact **server-reported** `data_directory` for
   that owned cluster. For native PostgreSQL this is the directory initialized
   with `initdb`; for Docker it is the path inside the server container, not the
   client host's volume path. The root guard compares it exactly and requires
   `180000 <= server_version_num < 190000` before Cargo, API, or migrations.
4. Verify database, role, address/port, directory, and system identifier against
   the cluster you created. Matching an arbitrary server's reported `PGDATA`
   does not make that server disposable; the guard is not ownership proof.

```bash
: "${TEST_DATABASE_URL:?Set the owned disposable PostgreSQL URL}"
: "${PGDATA:?Set the owned cluster's absolute server data_directory}"
psql "$TEST_DATABASE_URL" -X -v ON_ERROR_STOP=1 \
  -c 'SELECT current_database(),current_user,inet_server_addr(),inet_server_port();' \
  -c 'SHOW data_directory;' -c 'SHOW server_version_num;' \
  -c 'SELECT system_identifier FROM pg_control_system();'
export DATABASE_URL="$TEST_DATABASE_URL"
```

The URL/directory checks are test-harness safety rules, not production API
configuration. API startup applies migrations, so the explicit target matters
for a smoke launch too. Test roles need access to the identity query.

Root `test` builds the API first and obtains its executable path from Cargo's
JSON artifact output. It sets `RUNS_DECODER_EXECUTABLE` to that fresh binary for
in-process Rust HTTP/worker tests, then runs workspace Bun tests and serial
locked Rust tests. A test harness executable cannot act as the decoder child.
Root `test:e2e` also launches the freshly built API, installs Chromium, and
starts loopback API/Vite on ports **3000/5173**. Keep those ports free. Debug
test login is local-only and is not proof of real Google-provider login.

Alternatively, `bash scripts/test-postgres.sh` creates a unique Docker
PostgreSQL 18 container/volume with a random loopback port, sets server
`PGDATA=/var/lib/postgresql/18/docker`, invokes the guarded root test mode, and
cleans up only its owned resources. It runs tests; it is not a setup-only
command or a Runs reset tool.

For a standalone local API smoke, launch the built API binary, not a Rust test
harness. Core's decoder uses `RUNS_DECODER_EXECUTABLE` if explicitly set,
otherwise the running API executable; it must support `--runs-decode-child`.
Use matching current source and leave an inherited override unset. Private
spool directories/files require Unix ownership and modes 0700/0600 plus
bounded writable temporary storage. The configured child wall/CPU limits are
60 seconds and the workspace/document ceiling is 512 MiB; Linux also enforces
address-space limits. These ceilings do not prove RSS or production throughput.

The web development server proxies `/api` and `/healthz` to Axum at
`127.0.0.1:3000`. Production uses same-origin Google authentication and does
not configure CORS. Set Google OAuth variables before exposing the service
publicly and use TLS to protect the callback and session cookie.

## Preserved CLI

```bash
bun run --filter @garmin-fit-extractor/cli build
bun run --filter @garmin-fit-extractor/cli test
bun run --filter @garmin-fit-extractor/cli garmin-coach analyze activity.fit --output runs/activity.json
```

The CLI writes two-space JSON ending in a newline, preserves `schemaVersion: "1.0.0"`, and prints the absolute output path. Its JavaScript FIT SDK normalization contract remains unchanged.

## Runs workflow and API

Sign in with Google, upload 1–10 direct FIT or ZIP files, and inspect each
import outcome: `imported`, `duplicate`, `unsupported`, or `failed`. Supported
sources must pass CRC and FIT-profile checks identifying Garmin and exactly
one running session. A non-Garmin accessory does not by itself reject a
Garmin-recorded run. Other manufacturers, sports, and multi-session layouts
are explicitly unsupported; this is not a promise to accept every FIT file.
Valid ZIP members can succeed independently of invalid siblings.

Each uploaded file is limited to 20 MiB. The Core branch additionally bounds
expanded data to 100 MiB per batch, FIT members to 50 and ZIP members to 1,000,
and rejects unsafe paths, symlinks, nested archives, excessive ratios, and
resource overruns. These are implementation ceilings, not measured capacity
or timing claims.

An accepted source queues durable processing; import acceptance is not proof
that analysis is ready. Activity IDs remain stable across reprocessing.
Original FIT bytes remain immutable and owner-scoped; identical bytes are
deduplicated per owner. Decoded, normalized, and derived revisions remain
separate. A published manifest identifies one coherent revision set, not a
mixture of stages. Pending or failed updates can leave a clearly marked
last-good result visible. Version changes trigger reprocessing; manual
reprocess uses the stored original, not an exported JSON reconstruction.

History sorts by activity event time, not upload time. Detail shows Pace, HR,
Power, recorded laps, and separately detected workload segments. Missing
metrics remain unavailable, not synthetic zero. Display sampling and coach
aggregation disclose their transformations; they do not rewrite native
samples or full-resolution numerical inputs.

Authenticated Runs routes:

| Route | Purpose |
|---|---|
| `POST /api/v2/runs/imports` | Multipart `files`, direct FIT or ZIP; per-item batch results |
| `GET /api/v2/runs?limit=50&offset=0&sort=startTime&order=desc` | Owner-scoped history and processing state |
| `GET /api/v2/runs/{id}` | Coherent normalized detail, analysis, historical thresholds |
| `DELETE /api/v2/runs/{id}` | Permanent Runs erasure and dependent invalidation |
| `POST /api/v2/runs/{id}/reprocess` | Queue processing from the stored original |
| `GET /api/v2/runs/{id}/evidence` | Owner-only historical evidence and lineage |
| `GET /api/v2/runs/thresholds/latest` | Latest attempt and independently retained last LT1/LT2 values |
| `GET /api/v2/runs/thresholds/trend` | Event-time threshold history |
| `POST /api/v2/runs/exports` | Pin an exact selection, mode, privacy flags, and revision snapshot |
| `GET /api/v2/runs/exports/{token}` | Retrieve the pinned JSON bytes |
| `HEAD /api/v2/runs/exports/{token}` | Check authenticated metadata without reading the archive |

Errors use `{ "error": { "code": "...", "message": "..." } }` and an appropriate
HTTP status. The obsolete `/api/v1/extractions` collection, item, and download
APIs return `410 RUNS_ENDPOINT_RETIRED`; none is an unfiltered export fallback.
Source-less legacy rows retain their stable IDs and safe summaries with
`sourceUnavailable: true` and `LEGACY_SOURCE_UNAVAILABLE` fidelity warnings.
Their strict-v2 detail fields are null. Reprocessing returns
`409 SOURCE_UNAVAILABLE`; export returns `422 LEGACY_EXPORT_UNSUPPORTED`.
The service does not invent an original FIT or silently convert legacy JSON.

Deletion erases the activity's source and derived storage, cancels its work,
revokes affected exports, and invalidates downstream historical evidence.
Workers cannot republish a deleted activity. Files already copied or downloaded
before deletion cannot be recalled from the user's device or external AI.

## Selected JSON exports and manual AI handoff

Select one or many activities in History. Selection is independent of the
automatic history evidence and persists across pagination, filtering, and
ordering. Export requires the complete selection to be owned, present, ready,
and source-backed; invalid selections fail rather than produce a partial file.

- **Coach JSON** provides readable selected summaries, laps, segments, quality,
  thresholds, historical results, and boundary-preserving aggregated samples.
- **Full JSON** retains decoder-supported decoded fields, normalized samples,
  analysis, and historical results at full fidelity permitted by privacy.
  “Full” does not bypass privacy and is not a binary FIT download.

Both modes provide separate Copy and Download actions using the same pinned,
two-space pretty JSON bytes with a final newline and `schemaVersion: "2.0.0"`.
Copy uses the browser clipboard; Download uses the browser's file download
capability. A clipboard denial or size limit must not claim success: use the
same pinned download instead. No silent sample cap is permitted for Full JSON.
Browser acceptance of a download does not prove that the OS saved the file.

Export tokens are authenticated, owner-scoped, private/no-store, and expire
after **15 minutes**. A deletion revokes affected snapshots; retrieval checks
live activities and pinned revisions/history. HEAD does not reserve a lease
or guarantee a later GET. Revocation, expiry, timeout, or integrity loss during
a stream is a failed transfer, not a successful shortened JSON download.
Reprocessing can publish newer revisions without rewriting a still-valid
pinned snapshot. A new selection or changed privacy flags requires a new pin.

### Privacy defaults

Every export defaults to `includeLocation: false` and
`includeDeviceIdentifiers: false`. Location and device identifiers have
**independent opt-ins**; enabling one does not enable the other. Classification
checks numeric FIT identity and schema recursively, not just field names.
Unknown or unclassified native/developer fields remain omitted even when both
opt-ins are enabled.

Neither mode exports original FIT/base64, upload filenames, private paths,
account data, unselected activity IDs or raw contributor history, or the
account-wide trend. The `privacyOmissions` manifest records category, wildcard
schema position, count, and reason without hidden values. `transformations`
records actual aggregation or other changes. Privacy filtering never edits
the retained original or internal archive. Permitted metrics and event times
can still be sensitive; review the JSON before sharing it externally.

Copy editable ChatGPT or Claude prompt text separately, then attach or paste
the selected JSON yourself. Prompts are not embedded in JSON or PNG. Runs does
not call a runtime LLM, require an AI API key, or automatically send data to an
AI provider. The preserved FIT Coach OAuth integration is separate.

## Experimental threshold estimates

Device-reported thresholds remain separate from independent estimates. The
engine evaluates native recorded RR for DFA-a1 crossings at 0.75 and 0.50 as
experimental **VT1/VT2 proxies**, presented under LT1/LT2 target labels. These
are not validated blood-lactate measurements, medical advice, or automatic
training-zone prescriptions.

The open selector adaptation uses a contiguous alpha range `[0.5, 1.0]` with
at most one adjacent boundary window at each end, a negative slope, and an
observed crossing bracket. It does not extrapolate, infer maximal intent from
a workout name, reconstruct RR from HR, or manufacture one target from the
other. Missing alignment, continuity, protocol evidence, or a valid bracket
can produce target-specific abstention. See
[`docs/adr/0005-runs-threshold-methods.md`](docs/adr/0005-runs-threshold-methods.md)
for the method and its adaptation.

Each target reports `estimated`, `low_confidence`, or `insufficient_data`,
with actual method/version, evidence, trace, reasons, and optional collection
suggestions. Engine availability, processing failure, and freshness are
separate states. Suggestions are not a training plan or an all-out instruction.

Historical evidence uses the owner's activities ending at or before each
activity's cutoff, a seven-day recency policy, and distinct observation groups;
overlapping windows and duplicate copies are not independent runs. Checkbox
selection never controls this evidence. Late imports, deletion, and evidence
changes invalidate affected historical results. The latest view retains the
last numeric LT1 and LT2 independently, with their actual cutoff/computation
dates and stale flags; a newer LT2-only result does not hide an older LT1.

Synthetic fixtures and numerical comparator agreement can establish software
correctness, not physiological validity. Licensed paired human running RR
with independent gas-exchange references, participant-level holdout validation,
and empirical uncertainty remain missing release prerequisites for validated
VT claims. Lactate claims additionally need paired running lactate references.
Detector precision/recall also needs independent human workload annotations.

## Protected routes

The Axum API listens on `GARMIN_FIT_BIND` (default `0.0.0.0:3000`). The following
contracts remain separate from the Runs v2 cutover:

- `GET /api/v1/auth/login` and `GET /api/v1/auth/callback` for Google OAuth.
- `GET /api/v1/auth/me` and `POST /api/v1/auth/logout` for the current session.
- `/oauth/authorize` and `/oauth/token`, with `GET /api/v1/activities/latest`,
  `/api/v1/activities`, and `/api/v1/activities/{id}` for FIT Coach OAuth clients.
- `GET|POST|DELETE /api/admin/transcript-entries` and
  `GET|PUT|DELETE /api/admin/transcript-entries/{id}` for allowlisted sessions.
  Collection deletion requires `{ "confirmation": "DELETE_ALL" }`.
- `GET /healthz` (public).

Web routes `/` and `/shoes` are public entry points. `/upload`, `/history`, and
`/extractions/{id}` keep their authenticated route paths while using Runs v2.
`/shoes/{shoeId}` remains a public static catalog route. `/admin/transcripts`
appears in the authenticated shell, but API authorization remains authoritative.
See [`docs/admin-transcripts.md`](docs/admin-transcripts.md) for the admin workflow.

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

The isolated `legacy-data-migrator` is dry-run by default. Use only stopped, copied SQLite sources or standalone snapshots; never point it at a live Docker volume. Follow [`docs/postgresql-migration.md`](docs/postgresql-migration.md) for snapshot, prepare-target, dry-run, apply, verify, rollback, and sensitive-artifact handling. Reports contain counts and checksums only. This legacy migration is separate from Runs reprocessing; no migration apply or production cutover is claimed here.

Legacy source-less Runs rows remain read-only rather than being reset. No
startup, container recreation, deployment, or reprocess operation authorizes
a Runs reset. The existing reset design in
[`docs/runs-rebuild/architecture.md`](docs/runs-rebuild/architecture.md) is an
unexecuted proposal, not an available or exercised reset command. Apply would
require separate explicit authorization, a scoped dry-run digest, protected
data checks, and a tested restore. No legacy reset migration or Runs reset was
executed for this integration.


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

`apps/api/tests/fixtures/activity.fit` and its ZIP archive originate from
fitparser's MIT-licensed `tests/fixtures/Activity.fit`. This legacy fixture is
not Garmin-manufacturer data and is not a successful Runs v2 import oracle.
It remains useful for preserved CLI/legacy decoding.

Runs v2 fixtures and generators live in `apps/api/tests/fixtures/runs/`, with
license notices and manifests, including CC0 synthetic Garmin-running and
native RR cases. Capacity generators have their own manifest and license.
Synthetic fixtures do not represent licensed human physiological validation.
