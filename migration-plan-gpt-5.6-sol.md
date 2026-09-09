Note: plan.defaultOnStartup is ignored in print mode (no interactive surface to review the plan). Use --plan-yolo for a headless plan flow.
Working...
# 1. Target architecture

1. Keep the Garmin repository structure as the unified project:
   - `apps/web`: the only browser application.
   - `apps/api`: the only runtime API, implemented in Rust/Axum.
   - PostgreSQL 18: the only runtime database.
   - One deployed `garmin-fit-extractor` application service plus one `postgres` service.
   - Do not import the Running Shoe Elysia API, Drizzle layer, or separate admin Vite application.

2. Preserve Garmin routing and authentication:
   - `/`, `/upload`, `/history`, `/extractions/$id`: unchanged URLs and existing Google-session behavior.
   - Existing auth/login/callback routes: unchanged and public as required by their current flow.
   - Move the Garmin authentication requirement out of `__root.tsx` into a pathless authenticated layout so public shoe routes do not redirect to Google authentication.

3. Add public static catalog routes:
   - `/shoes`: catalog and recommendation UI.
   - `/shoes/$shoeId`: product detail, reviews, and size-chart UI.
   - The `/` collision is resolved in Garmin’s favor. Garmin retains `/`; the imported catalog receives `/shoes`.
   - Keep products, reviews, size charts, recommendation inputs, and recommendation logic in checked-in frontend TypeScript. Add no catalog database tables or catalog API endpoints.
   - Copy shoe images into the Garmin web public directory without renaming unless a filename collision requires a catalog-reference update.

4. Integrate private intake administration into the same Garmin web/API pair:
   - UI: `/admin/transcripts`.
   - API:
     - `GET /api/admin/transcript-entries`
     - `POST /api/admin/transcript-entries`
     - `PUT /api/admin/transcript-entries/{id}`
     - `DELETE /api/admin/transcript-entries/{id}`
   - Require a valid existing Garmin Google session and membership in `ADMIN_EMAILS`.
   - Treat an absent or empty `ADMIN_EMAILS` value as deny-all.
   - Normalize configured and authenticated email addresses with trim plus ASCII lowercase, and require the verified Google email represented by the existing authenticated user.
   - Enforce authorization in Axum for every endpoint. Client-side route guards and hidden navigation are presentation only.
   - Preserve duplicate `video_id` behavior as HTTP `409 Conflict`.
   - Keep transcription text out of public/static data and unauthenticated responses.
   - Keep the admin paths off public navigation for non-admin users. The deployment proxy/firewall should additionally limit `/admin/*` and `/api/admin/*` to the intended intranet, but network filtering must not replace server authorization.

5. Copy source material only from the two original repositories. Modify files only under the new Garmin worktree. Do not introduce commits, merges, or pushes as part of implementation.

---

# 2. File-level change matrix

## 2.1 Root, build, deployment, and documentation

| Path | Action | Required change |
|---|---|---|
| `Cargo.toml` | Change | Add `tools/legacy-data-migrator` to the Cargo workspace. |
| `Cargo.lock` | Regenerate | Record PostgreSQL and isolated migration-tool dependencies. |
| `package.json` | Change if needed | Ensure existing aggregate `build`, `test`, and `check` scripts cover the unified web and Cargo workspace. Do not add a second frontend workspace. |
| `docker-compose.yml` | Replace runtime topology | Keep application service, remove SQLite volume wiring, add PostgreSQL 18 and the `intranet` network. |
| `Dockerfile` | Change | Remove runtime SQLite/data-volume assumptions; continue building the Rust API and Garmin web bundle as one deployable application. |
| `.env.example` | Change/add | Document non-secret placeholders for PostgreSQL and admin configuration. |
| `.gitignore` | Change | Ignore `/db-data/`, `/migration-artifacts/`, `.env`, database dumps, and SQLite snapshots. |
| `.dockerignore` | Change | Exclude `db-data`, migration artifacts, snapshots, dumps, `.env`, and original-repository paths from build context. |
| `README.md` | Change | Document unified routes, PostgreSQL development setup, static-catalog ownership, and private admin access. |
| `docs/postgresql-migration.md` | Add | Exact operator runbook for snapshot, dry run, import, verification, cutover, and rollback. |
| `scripts/test-postgres.sh` | Add | Start an isolated PostgreSQL 18 test instance, run migrations and PostgreSQL integration tests, and remove it with a trap. No fixed real credential. |

## 2.2 Rust API and PostgreSQL schema

| Path | Action | Required change |
|---|---|---|
| `apps/api/Cargo.toml` | Change | Replace SQLx `sqlite` with `postgres`; retain `runtime-tokio`, `migrate`, and required UUID support. Add only the time dependency needed to generate the existing UTC timestamp representation. |
| `apps/api/src/config.rs` | Change | Reject `sqlite:` URLs; accept only `postgres://` and `postgresql://`. Parse and validate `ADMIN_EMAILS`. Add tests for accepted/rejected schemes and deny-all admin configuration. |
| `apps/api/src/db.rs` | Change | Replace `SqlitePool`/SQLite transactions and all SQLite SQL with `PgPool`/PostgreSQL SQL. Preserve existing API models and serialized values. |
| `apps/api/src/app.rs` | Change | Store `PgPool`, mount admin transcript routes, and apply existing session middleware to the new protected endpoints. |
| `apps/api/src/main.rs` | Change | Connect with `PgPoolOptions`, run embedded PostgreSQL migrations, and start only after the pool and migrations are ready. |
| `apps/api/src/admin.rs` | Add | Admin authorization, transcript request/response models, validation, CRUD handlers, duplicate mapping, and router construction. Reuse existing session lookup rather than creating another auth system. |
| `apps/api/migrations/0001_extractions.sql` | Rewrite | PostgreSQL form of the existing extraction/activity schema. |
| `apps/api/migrations/0002_google_users_sessions_zip_history.sql` | Rewrite | PostgreSQL form of users, sessions, OAuth, and history schema. |
| `apps/api/migrations/0003_fit_coach.sql` | Rewrite | PostgreSQL form of every table/index/constraint currently introduced by this migration. |
| `apps/api/migrations/0004_transcript_entries.sql` | Add | Private transcript queue schema and indexes. |
| `apps/api/migrations/0005_legacy_imports.sql` | Add | Import ledger used to make the one-time migration auditable and idempotent. |
| `apps/api/tests/postgres_api.rs` | Add/update | Existing Garmin DB behavior against PostgreSQL, including history, extraction, session, OAuth, and timestamp behavior. |
| `apps/api/tests/admin_transcript_entries.rs` | Add | Auth failures, allowlist enforcement, CRUD, duplicate `video_id`, unchanged `created_at`, updated `updated_at`, and delete behavior. |

The rewritten `0001`–`0003` migrations are a clean PostgreSQL baseline for a new database. They are not to be applied to an existing SQLite `_sqlx_migrations` history. SQLite remains only as a read-only source format consumed by the isolated migration tool.

## 2.3 Unified Garmin web

Use the repository’s existing TanStack flat-route naming convention.

| Path | Action | Required change |
|---|---|---|
| `apps/web/src/routes/__root.tsx` | Change | Retain providers, document shell, auth-state loading, and global styles; remove unconditional authenticated-route redirect. |
| `apps/web/src/routes/_authenticated.tsx` | Add | Pathless layout containing the existing Garmin authentication requirement and authenticated shell. |
| `apps/web/src/routes/_authenticated.index.tsx` | Move/adapt | Existing Garmin `/` implementation. |
| `apps/web/src/routes/_authenticated.upload.tsx` | Move/adapt | Existing `/upload` implementation. |
| `apps/web/src/routes/_authenticated.history.tsx` | Move/adapt | Existing `/history` implementation. |
| `apps/web/src/routes/_authenticated.extractions.$id.tsx` | Move/adapt | Existing `/extractions/$id` implementation. |
| Existing pre-move Garmin route files | Remove | Remove only after every route has moved to the authenticated layout and retained the same URL. |
| `apps/web/src/routes/shoes.index.tsx` | Add | Public catalog route. |
| `apps/web/src/routes/shoes.$shoeId.tsx` | Add | Public product detail route; unknown IDs produce the router’s normal not-found behavior. |
| `apps/web/src/routes/_authenticated.admin.transcripts.tsx` | Add | Authenticated admin route; perform admin-status loading but rely on API authorization. |
| `apps/web/src/routeTree.gen.ts` | Regenerate | Record all moved and new TanStack routes. |
| `apps/web/src/data/catalog.ts` | Add | Copy the complete Git-tracked running-shoe catalog unchanged except import paths or image-base paths required by the unified app. |
| `apps/web/src/domain/catalog.ts` | Add | Copy catalog lookup/validation logic. |
| `apps/web/src/domain/recommendations.ts` | Add | Copy recommendation behavior and preserve existing outputs. |
| `apps/web/src/features/shoes/ShoeCatalogPage.tsx` | Add | Garmin-styled catalog, filters/recommendations, and responsive list/grid. |
| `apps/web/src/features/shoes/ShoeDetailPage.tsx` | Add | Product facts, size chart, reviews, recommendation context, and image handling. |
| `apps/web/src/features/shoes/ShoeCard.tsx` | Add | Reusable accessible card/link presentation. |
| `apps/web/src/features/shoes/RecommendationPanel.tsx` | Add | Adapt the existing recommendation controls without changing recommendation rules. |
| `apps/web/src/features/admin/TranscriptEntriesPage.tsx` | Add | Authorized queue list, create/edit/delete states, conflict/error handling. |
| `apps/web/src/features/admin/TranscriptEntryForm.tsx` | Add | Labeled controlled form for the four operator-entered fields. |
| `apps/web/src/api/transcriptEntries.ts` | Add | Typed same-origin API client using the existing fetch/error/session conventions. |
| `apps/web/src/index.css` | Change | Add catalog/detail/admin component styles using existing semantic tokens only. |
| `apps/web/src/domain/catalog.test.ts` | Add/copy | Lookup, unique IDs, valid references, and route resolution. |
| `apps/web/src/domain/recommendations.test.ts` | Add/copy | Preserve recommendation inputs, ranking, boundaries, and deterministic behavior. |
| `apps/web/src/data/catalog.test.ts` | Add | Validate data shape, size-chart ordering, referenced shoe IDs, image paths, and image-file existence. |
| `apps/web/public/images/shoes/**` | Add | Copy every catalog image used by `catalog.ts`; preserve case and relative filenames. |

Keep existing public auth callback/login route files outside `_authenticated`. Do not move a route into the authenticated layout if doing so would block the Google login callback.

## 2.4 One-time migration utility

| Path | Action | Required change |
|---|---|---|
| `tools/legacy-data-migrator/Cargo.toml` | Add | Isolated CLI dependencies: read-only SQLite access, PostgreSQL SQLx, SHA-256, serialization, error handling, and CLI parsing. |
| `tools/legacy-data-migrator/src/main.rs` | Add | `snapshot`, `prepare-target`, `import`, and `verify` commands. Dry-run is the default; mutation requires `--apply`. |
| `tools/legacy-data-migrator/src/source.rs` | Add | Open SQLite with read-only flags, validate expected schema, run integrity checks, and read every source table explicitly. |
| `tools/legacy-data-migrator/src/target.rs` | Add | PostgreSQL inserts, target rereads, sequence repair, advisory lock, ledger handling, and transaction boundaries. |
| `tools/legacy-data-migrator/src/checksum.rs` | Add | Canonical type- and length-delimited row hashing. |
| `tools/legacy-data-migrator/src/report.rs` | Add | Machine-readable and human-readable count/checksum report without row contents or credentials. |
| `tools/legacy-data-migrator/tests/import.rs` | Add | Generated synthetic SQLite fixtures covering both schemas, exact preservation, rerun behavior, mismatch failure, rollback, and sequences. |

Do not commit production snapshots, migration reports containing sensitive paths, database dumps, credentials, session values, OAuth values, or transcription contents.

---

# 3. PostgreSQL schema and query conversion

## 3.1 Schema rules

1. Translate every existing Garmin table, column, index, foreign key, uniqueness constraint, and deletion rule from migrations `0001`–`0003`. Do not omit empty OAuth tables; they are part of the application contract and future state.

2. Preserve representations deliberately:
   - Existing UUID IDs stored as SQLite text remain PostgreSQL `TEXT`, not `UUID`. This avoids canonicalization or rejection of historical text.
   - Existing JSON-bearing text columns remain `TEXT`, not `JSONB`. The migration must preserve the original bytes and application serialization behavior.
   - Existing textual timestamps remain `TEXT`; historical values are copied byte-for-byte.
   - New writes to those columns must use the same UTC RFC3339 millisecond format previously produced by SQLite `strftime`.
   - Running-shoe `created_at` and `updated_at` remain integer Unix milliseconds, represented as PostgreSQL `BIGINT`.
   - SQLite `BLOB` becomes `BYTEA`.
   - SQLite `REAL` becomes `DOUBLE PRECISION`.
   - General SQLite integer values become `BIGINT` unless the source column is a proven logical boolean. A boolean conversion must map only `0`/`1` and reject other historical values.
   - Preserve nullable versus non-nullable behavior and defaults. Do not add JSON-validity or stricter content checks that could reject preserved data.

3. Integer-generated IDs use:

```sql
id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY
```

`BY DEFAULT` permits explicit historical IDs during import.

4. Add the transcript table:

```sql
CREATE TABLE transcript_entries (
    id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    channel_name TEXT NOT NULL,
    youtube_url TEXT NOT NULL,
    video_id TEXT NOT NULL UNIQUE,
    transcription TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE INDEX transcript_entries_updated_at_id_idx
    ON transcript_entries (updated_at DESC, id DESC);
```

5. Add an import ledger:

```sql
CREATE TABLE legacy_imports (
    source_name TEXT PRIMARY KEY,
    source_sha256 TEXT NOT NULL,
    imported_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    table_manifest JSONB NOT NULL
);
```

Use one ledger row for the Garmin snapshot and one for the intake snapshot. Both rows are committed in the same transaction as all imported data.

## 3.2 SQL and Rust compatibility changes

Convert all affected SQL explicitly:

- `?` placeholders → `$1`, `$2`, etc.
- `INSERT OR IGNORE` → `INSERT ... ON CONFLICT (<actual constraint columns>) DO NOTHING`.
- Any `INSERT OR REPLACE` → explicit `ON CONFLICT ... DO UPDATE`, preserving only the previous update semantics.
- `last_insert_rowid()` → `INSERT ... RETURNING id`.
- SQLite `changes()` → SQLx `rows_affected()`.
- `strftime(..., 'now')` → a single Rust-generated UTC RFC3339 millisecond string bound to the statement.
- Unix timestamp generation → a Rust-generated `i64` millisecond value.
- Case-insensitive user search that relied on SQLite `LIKE` → PostgreSQL `ILIKE`.
- Exact token, UUID, session, OAuth-state, and `video_id` matching remains `=`.
- Add explicit `NULLS FIRST` or `NULLS LAST` wherever current observable ordering depends on SQLite’s null ordering.
- Replace `SqlitePool`, `SqlitePoolOptions`, `Transaction<'_, Sqlite>`, and SQLite row types with PostgreSQL equivalents.
- Use `RETURNING` for create/update operations instead of insert-then-select races.
- Preserve all transaction boundaries and make multi-table writes use one PostgreSQL transaction.
- Map PostgreSQL unique-constraint violations to the existing domain conflict result. For transcript `video_id`, return `409`; do not expose raw database error text.
- Do not depend on SQLite’s permissive type coercion. Bind Rust values with their intended PostgreSQL types.
- Preserve parent-before-child insert ordering and existing foreign-key actions.

## 3.3 Admin CRUD behavior

- `GET`: stable order by `updated_at DESC, id DESC`.
- `POST`: server generates `created_at` and `updated_at` once from the same millisecond value; identity generates the ID.
- `PUT`: update `channel_name`, `youtube_url`, `video_id`, and `transcription`; preserve `created_at`; generate a new `updated_at`.
- `DELETE`: hard delete, matching the existing queue semantics.
- Validate required non-empty fields after trimming for validation. Do not silently rewrite the stored transcription.
- Return structured errors using the existing API error envelope.
- Enforce same-origin/CSRF protections already used by authenticated Garmin mutations. If none exists, reject mutating admin requests whose `Origin` does not match the configured application origin.

---

# 4. Garmin-first frontend integration

1. Treat root `tokens.css` and `apps/web/src/index.css` as the only visual foundation:
   - Use Bricolage Grotesque for display hierarchy, IBM Plex Sans for body/UI, and JetBrains Mono where structured values require it.
   - Use the existing Catppuccin semantic colors and Garmin zone palette.
   - Do not copy Running Shoe global tokens, font declarations, reset, page chrome, raw theme colors, or parallel button/card systems.
   - Extend an existing Garmin component pattern before adding a shoe-specific variant.

2. Catalog:
   - Garmin header and page-width conventions.
   - Responsive one-column layout at narrow widths and existing grid breakpoints at larger widths.
   - Entire shoe card has one clear link target; avoid nested interactive elements.
   - Recommendation controls retain labels, keyboard operation, visible focus, validation, and deterministic results.
   - Empty/no-match state explains how to broaden inputs.

3. Detail:
   - Preserve product facts, reviews, and size-chart content from static data.
   - Render size charts as semantic tables with captions and horizontal overflow on small screens.
   - Use descriptive image alt text from catalog data.
   - Set intrinsic `width`/`height` or CSS `aspect-ratio` to prevent layout shift.
   - Use `object-fit` without cropping information-critical content.
   - Lazy-load card and secondary images; load the detail hero eagerly.
   - Render a designed fallback when an image fails rather than a broken image icon.

4. Admin:
   - Use the same Garmin shell, form controls, cards/tables, spacing, focus rings, status colors, and error treatment.
   - On narrow screens, switch queue rows to stacked labeled records rather than compressing columns.
   - Confirm deletion with a focused accessible dialog or the project’s existing confirmation pattern.
   - Announce save/delete/error results through an `aria-live` status region.
   - Never place transcription values in URLs, local storage, analytics events, or console logs.

5. Public/private route behavior:
   - `/shoes` and `/shoes/$shoeId` must render without a session.
   - Existing Garmin routes must still require the same session behavior.
   - `/admin/transcripts` may show a login flow for unauthenticated users and a forbidden state for authenticated non-admins.
   - API `401`/`403` remains authoritative even if the client has stale admin state.

---

# 5. One-time migration design

## 5.1 Utility behavior

1. `snapshot`:
   - Open the copied SQLite source with read-only flags.
   - Refuse a writable source.
   - Run `PRAGMA quick_check` and `PRAGMA foreign_key_check`.
   - Validate required tables and columns against the expected Garmin or intake schema.
   - Use SQLite’s backup API to produce a standalone, consistent snapshot, including committed WAL-visible data.
   - Emit the snapshot SHA-256 and source schema manifest.
   - Never open the live Docker volume read-write.

2. `prepare-target`:
   - Connect only to PostgreSQL.
   - Run the embedded `apps/api/migrations` migration set.
   - Verify the PostgreSQL migration history and required table definitions.
   - Refuse SQLite URLs.

3. `import`:
   - Require both the Garmin and intake snapshots so the production import is all-or-nothing.
   - Default to dry-run. Require `--apply` for commit.
   - Begin a `SERIALIZABLE` transaction and acquire a fixed PostgreSQL advisory lock.
   - Read every table explicitly; do not use a generic column copier.
   - Insert parent tables before children.
   - Preserve every primary key, text UUID, string, timestamp, JSON text, blob, null, foreign key, and transcript integer timestamp.
   - Use `ON CONFLICT DO NOTHING`, then compare the target row by primary key with the complete source row.
   - Identical existing rows are accepted; any mismatch aborts the complete transaction.
   - An alternate unique-key conflict with another primary key aborts the transaction.
   - Do not overwrite target data to force a successful rerun.
   - After import, reread every target table and compare row count plus canonical checksum.
   - Reset each identity sequence to the table’s maximum imported ID. For an empty table, retain its initial sequence state.
   - Insert both `legacy_imports` rows only after all comparisons pass.
   - Dry-run performs all inserts and checks inside the transaction and then rolls back.
   - Apply commits only after all checks pass.

4. `verify`:
   - Recompute source and target count/checksum manifests without mutation.
   - Verify ledger hashes.
   - Verify every identity sequence is positioned at or above its table maximum.
   - Verify foreign-key consistency.
   - Exit nonzero on any discrepancy.

5. Idempotency:
   - Reapplying the same two snapshots must find matching ledger hashes and matching target manifests, perform no writes, and succeed.
   - A different snapshot hash under an already-imported source name must fail. It must not silently append or replace data.
   - A failed import leaves no table rows, sequence changes, or ledger rows because all import changes share one transaction.

## 5.2 Canonical checksum format

For each table:

1. Read all columns in declared schema order.
2. Serialize each value with:
   - explicit type marker;
   - null marker;
   - byte length;
   - raw UTF-8 or binary bytes.
3. Sort rows in Rust by the canonical primary-key byte representation, avoiding SQLite/PostgreSQL collation differences.
4. Feed the table name, column names, and serialized rows into SHA-256.
5. Store and report only table name, row count, and digest.

This preserves distinctions such as null versus empty string, integer `1` versus text `"1"`, and JSON whitespace. Reports must not print sessions, OAuth values, URLs, transcription text, or other row fields.

---

# 6. Operator migration and cutover procedure

These are future operator commands and gates, separate from source implementation.

## 6.1 Freeze and immutable backups

1. Schedule a no-write window.
2. Stop both old application stacks or otherwise prove their SQLite writers are stopped.
3. Do not start the unified application yet.
4. Create a private ignored directory:

```bash
install -d -m 0700 migration-artifacts/raw migration-artifacts/snapshots migration-artifacts/reports
```

5. Archive each complete volume through a read-only mount, including database, WAL, SHM, and any adjacent files:

```bash
docker run --rm \
  -v garmin_fit_data:/source:ro \
  alpine:3.22 \
  tar -C /source -cf - . \
  > migration-artifacts/raw/garmin-volume.tar

docker run --rm \
  -v running-shoe-size-advisor-admin-data:/source:ro \
  alpine:3.22 \
  tar -C /source -cf - . \
  > migration-artifacts/raw/running-shoe-intake-volume.tar
```

6. Hash, restrict, and separately back up both archives:

```bash
sha256sum migration-artifacts/raw/*.tar \
  > migration-artifacts/raw/SHA256SUMS
chmod 0400 migration-artifacts/raw/*.tar migration-artifacts/raw/SHA256SUMS
```

7. Extract into private staging directories. Point the migration tool at the actual SQLite files found in those extracted copies, never at `/var/lib/docker/volumes/...` and never at the mounted live volumes.
8. Run `snapshot` for each source to produce standalone SQLite snapshots.
9. Preserve both raw-volume archives and standalone snapshot hashes until after cutover acceptance.

## 6.2 PostgreSQL preparation

1. Create an ignored `.env` using generated credentials. Use a URL-safe generated password or correctly percent-encode it in `DATABASE_URL`.

Required variables:

```dotenv
POSTGRES_DB=garmin_fit_extractor
POSTGRES_USER=garmin_fit_extractor
POSTGRES_PASSWORD=<generated-secret>
DATABASE_URL=postgresql://garmin_fit_extractor:<encoded-secret>@postgres:5432/garmin_fit_extractor
ADMIN_EMAILS=<comma-separated-verified-google-emails>
```

2. Create the bind directory:

```bash
install -d -m 0700 db-data
```

3. Validate Compose with non-production placeholder values before using real credentials:

```bash
POSTGRES_DB=test \
POSTGRES_USER=test \
POSTGRES_PASSWORD=test \
DATABASE_URL=postgresql://test:test@postgres:5432/test \
ADMIN_EMAILS=admin@example.invalid \
docker compose config
```

4. Start only PostgreSQL:

```bash
docker compose up -d postgres
```

5. Wait for the Compose PostgreSQL healthcheck.
6. Run `prepare-target`.
7. Take a schema-only `pg_dump` before import for an additional audit artifact.

## 6.3 Dry run, apply, and verification

1. Run the importer without `--apply`. Save the count/checksum report.
2. Require the production dry-run report to include at least the known source baseline:
   - Garmin users: `1`
   - Garmin sessions: `3`
   - Garmin extractions: `11`
   - Garmin activities: `11`
   - Inspected OAuth transient tables: `0`
   - Intake transcript entries: `1`, preserving primary key `56`
3. Also require reports for every other table in migrations `0001`–`0003`, even when its count is zero.
4. Investigate any count different from the frozen source. Do not edit the importer to suppress a discrepancy.
5. Run the same command with `--apply`.
6. Run `verify`.
7. Run the same `--apply` import a second time:
   - it must report an already imported identical snapshot;
   - it must leave counts, checksums, sequences, and ledger timestamps unchanged.
8. Take a PostgreSQL custom-format dump after successful import:

```bash
pg_dump --format=custom --file=migration-artifacts/post-import.dump "$DATABASE_URL"
chmod 0400 migration-artifacts/post-import.dump
sha256sum migration-artifacts/post-import.dump \
  > migration-artifacts/post-import.dump.sha256
```

Use a host-reachable PostgreSQL URL for host-side commands; do not copy the Compose-internal hostname literally if it does not resolve on the host.

## 6.4 Cutover

1. Start the unified application only after import verification passes:

```bash
docker compose up -d garmin-fit-extractor
```

2. Keep old SQLite applications stopped and their volumes untouched.
3. Before allowing mutating traffic:
   - check PostgreSQL and application health;
   - load `/shoes` and one known detail page;
   - confirm existing unauthenticated Garmin route behavior;
   - confirm an unauthenticated admin API request is denied;
   - confirm a non-admin authenticated request is forbidden;
   - inspect imported history and extraction details using the intended admin/operator account;
   - confirm the authorized intake row appears with ID `56`.
4. Switch reverse-proxy/DNS traffic only after those checks pass.
5. Preserve old volumes and migration artifacts according to the project’s backup retention policy.

## 6.5 Rollback

Before any new unified-application writes:

1. Remove traffic from the unified app.
2. Stop the unified app and PostgreSQL.
3. Preserve, do not delete, the PostgreSQL dump and `db-data`; rename the directory with a failed-cutover timestamp if a fresh retry is needed.
4. Restore traffic to the old applications using their untouched original images/configuration and untouched SQLite volumes.
5. Diagnose from reports and logs without modifying the source backups.

After PostgreSQL accepts new application writes, direct rollback to SQLite is no longer safe because this plan deliberately contains no reverse synchronizer. From that point, use a roll-forward repair or design and validate a separate reverse migration before restoring old services.

---

# 7. Compose and environment specification

`docker-compose.yml` must have this effective topology:

```yaml
services:
  postgres:
    image: postgres:18
    environment:
      POSTGRES_DB: ${POSTGRES_DB:?POSTGRES_DB is required}
      POSTGRES_USER: ${POSTGRES_USER:?POSTGRES_USER is required}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:?POSTGRES_PASSWORD is required}
    ports:
      - "5432:5432"
    volumes:
      - ./db-data:/var/lib/postgresql
    healthcheck:
      test:
        [
          "CMD-SHELL",
          "pg_isready -U \"$${POSTGRES_USER}\" -d \"$${POSTGRES_DB}\""
        ]
      interval: 5s
      timeout: 5s
      retries: 12
      start_period: 10s
    networks:
      - intranet

  garmin-fit-extractor:
    # Preserve the existing build/image and 8100:3000 mapping.
    environment:
      DATABASE_URL: ${DATABASE_URL:?DATABASE_URL is required}
      ADMIN_EMAILS: ${ADMIN_EMAILS:-}
      # Preserve all existing Garmin auth/OAuth/session environment variables.
    depends_on:
      postgres:
        condition: service_healthy
    networks:
      - intranet

networks:
  intranet:
    name: intranet
```

Additional requirements:

- Remove the `garmin_fit_data` runtime mount and top-level SQLite named-volume declaration.
- Do not add the Running Shoe SQLite volume to the unified Compose file.
- Do not commit `.env` or interpolate real values into Compose.
- Retain the current application healthcheck.
- PostgreSQL readiness and application health are separate gates.
- Because `5432:5432` publishes PostgreSQL, manually confirm the host firewall limits it to the intended intranet. Do not weaken PostgreSQL authentication.
- Inspect the archived Garmin volume for non-database application files before deleting its application mount. If extraction records reference persisted files, copy those files into a dedicated non-database bind mount and preserve their paths. This is a cutover blocker; do not assume the volume contained only SQLite.
- Do not mount `db-data` into the application service or include it in Docker build context.

---

# 8. Verification plan

## 8.1 Static and build checks

Run from the unified worktree:

```bash
bun install --frozen-lockfile
bun run check
bun run test
bun run build

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

Then run the PostgreSQL-specific harness:

```bash
./scripts/test-postgres.sh
```

The harness must use `postgres:18`, generated temporary credentials, a disposable volume/container, and cleanup on success or failure.

## 8.2 Required automated behavior

1. Existing Garmin API and web tests pass after PostgreSQL conversion.
2. Configuration tests:
   - accept `postgres://` and `postgresql://`;
   - reject `sqlite://`, malformed URLs, and missing database configuration;
   - empty `ADMIN_EMAILS` denies admin access.
3. PostgreSQL query tests cover:
   - inserts and returned IDs;
   - conflict-ignore behavior;
   - update and delete row counts;
   - timestamp wire format;
   - nullable ordering;
   - session/OAuth token exact matching;
   - extraction/activity relationships.
4. Admin API tests cover:
   - `401` unauthenticated;
   - `403` authenticated but not allowlisted;
   - successful list/create/update/delete;
   - duplicate `video_id` → `409`;
   - no database details in conflict responses;
   - `created_at` stability and `updated_at` change.
5. Catalog tests cover:
   - unique and URL-safe shoe IDs;
   - every image reference exists with exact case;
   - every review/size-chart/recommendation reference resolves;
   - valid ordered size-chart values;
   - recommendation outputs remain deterministic;
   - unknown shoe IDs return not-found behavior.
6. Migration fixture tests cover:
   - explicit integer IDs, including a non-contiguous high ID such as `56`;
   - UUID-like text preserved exactly;
   - Unicode, empty strings, nulls, blobs, and JSON whitespace;
   - textual Garmin timestamps and integer intake timestamps;
   - parent/child foreign keys;
   - first apply;
   - identical second apply with no changes;
   - changed source hash refusal;
   - conflicting existing row rollback;
   - complete transaction rollback after a late-table failure;
   - sequence next value greater than the imported maximum;
   - dry-run rollback;
   - reports that contain no row contents.

## 8.3 Compose and runtime checks

```bash
docker compose config
docker compose up -d postgres
docker compose ps
docker compose up -d garmin-fit-extractor
docker compose ps
```

Runtime smoke checks:

- PostgreSQL health becomes healthy through `pg_isready`.
- Application health endpoint becomes healthy.
- `/shoes` returns the unified web application without authentication.
- A valid `/shoes/$shoeId` renders its catalog data and image.
- An invalid shoe ID shows not-found behavior.
- `/`, `/upload`, `/history`, and `/extractions/$id` retain existing Garmin authentication behavior.
- Existing imported history shows 11 extractions and associated activities.
- Public requests cannot retrieve transcript data.
- Admin APIs return `401`/`403` appropriately.
- An allowlisted user can list and edit the preserved transcript entry.
- Restarting both services retains PostgreSQL data in `./db-data`.
- Deep-link requests to shoe and admin routes reach the SPA rather than a server 404.
- No deployed process opens or creates a SQLite database.

## 8.4 Manual visual/accessibility checks

Check at narrow mobile, tablet, and desktop widths:

- no horizontal page overflow except the intentional size-table scroller;
- shoe images retain aspect ratio and do not shift surrounding layout;
- all controls and links are keyboard reachable in logical order;
- visible focus is present;
- forms have persistent labels and associated error text;
- recommendation and admin status changes are announced;
- color contrast remains valid under Garmin tokens;
- reduced-motion preferences are respected;
- delete confirmation returns focus correctly;
- long channel names, URLs, and transcription content wrap without breaking the admin layout.

---

# 9. Risks and cutover blockers

1. **Authentication-layout regression:** moving the auth guard from `__root.tsx` can accidentally expose Garmin routes or block callback routes. Route tests and direct URL smoke checks are mandatory.

2. **Admin exposure:** hiding navigation is insufficient. Confirm every admin API handler performs server-side session plus allowlist authorization. Confirm the reverse proxy’s intranet restriction separately.

3. **PostgreSQL semantic differences:** placeholder syntax, null ordering, case-insensitive search, identity values, timestamps, and permissive SQLite typing can change behavior. Each affected query must be translated, not mechanically string-replaced.

4. **Historical text fidelity:** converting UUID text to PostgreSQL UUID, JSON text to JSONB, or textual timestamps to `TIMESTAMPTZ` would change stored representation. Keep those historical application columns as `TEXT`.

5. **Sequence drift:** explicit imported IDs do not automatically advance identity sequences. Import verification must inspect every generated-ID sequence.

6. **SQLite WAL consistency:** copying only the main `.db` file can lose committed rows. Archive the whole stopped volume and use the backup API on the copied database.

7. **Sensitive backups:** Garmin sessions/OAuth values and intake transcription are sensitive. Use private permissions, encrypted backup storage, no console row dumps, and no committed artifacts.

8. **Non-database files in the Garmin volume:** inspect the archived volume and database references before removing `/data`. Any persisted FIT/upload artifact requires a dedicated migration and mount.

9. **Existing session continuity:** manually confirm that session-signing/encryption secrets, cookie domain, secure-cookie settings, public origin, and Google callback URLs are preserved. Database row migration alone does not preserve sessions if deployment secrets change.

10. **PostgreSQL 18 bind mount:** confirm filesystem ownership, free space, backup policy, and recovery procedure for `./db-data:/var/lib/postgresql` on the deployment host.

11. **Published database port:** `5432:5432` follows the required shape but increases exposure. Confirm firewall and intranet routing before cutover.

12. **Image portability:** Linux paths are case-sensitive. Verify every catalog image reference against the built artifact, not only the development server.

13. **Rollback boundary:** no writes may reach PostgreSQL until pre-cutover verification passes. Once writes begin, rollback to the untouched SQLite services would discard new state and is prohibited without a separately validated reverse migration.

---

# 10. Final acceptance checklist

- [ ] All implementation changes exist only in the new Garmin worktree.
- [ ] Original Garmin and Running Shoe checkouts remain unmodified.
- [ ] Garmin remains the single web/API project and visual system.
- [ ] No separate Running Shoe web, admin, Elysia API, Drizzle runtime, or SQLite runtime is deployed.
- [ ] `/`, `/upload`, `/history`, and `/extractions/$id` retain their existing URLs and authentication behavior.
- [ ] `/shoes` and `/shoes/$shoeId` are public.
- [ ] Catalog products, reviews, size charts, recommendation logic, and images remain checked-in frontend data/assets.
- [ ] No catalog table or catalog API endpoint exists.
- [ ] `/admin/transcripts` uses the unified Garmin web application.
- [ ] Transcript CRUD uses the Rust API and PostgreSQL.
- [ ] Every admin API operation requires a valid Garmin session and `ADMIN_EMAILS` membership.
- [ ] Empty admin configuration denies everyone.
- [ ] PostgreSQL migrations cover every Garmin table plus `transcript_entries` and `legacy_imports`.
- [ ] UUID text, JSON text, timestamps, IDs, nulls, unique constraints, and foreign keys are preserved.
- [ ] The migration imports both SQLite sources in one transaction.
- [ ] Dry-run rolls back.
- [ ] Applying the same snapshots twice is a no-op on the second run.
- [ ] A mismatched source or target row aborts rather than overwriting data.
- [ ] Source and target row counts and canonical checksums match for every table.
- [ ] Known production counts match the frozen baseline.
- [ ] Transcript primary key `56` is preserved.
- [ ] Every identity sequence is repaired and verified.
- [ ] Raw transcript, session, OAuth, and credential values never appear in migration reports.
- [ ] `postgres:18` uses `./db-data:/var/lib/postgresql`.
- [ ] PostgreSQL has the required environment variables, port, healthcheck, and `intranet` network.
- [ ] The application waits for PostgreSQL health and uses only PostgreSQL at runtime.
- [ ] No real credentials, `.env`, SQLite snapshot, dump, or `db-data` content is committed or included in the image.
- [ ] Rust formatting, lint, unit, integration, and migration tests pass.
- [ ] Bun check, tests, and production build pass.
- [ ] Compose configuration and both runtime healthchecks pass.
- [ ] Catalog, Garmin authenticated routes, imported history, admin authorization, responsive layout, accessibility, images, restart persistence, and SPA deep links pass runtime checks.
- [ ] Immutable source backups and post-import PostgreSQL dump exist before traffic cutover.
- [ ] No commit, merge, or push is performed.
