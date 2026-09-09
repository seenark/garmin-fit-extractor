# SQLite to PostgreSQL migration

This runbook is for the one-time cutover from stopped SQLite copies to the unified Garmin PostgreSQL service. The migrator is read-only by default. It never opens a Docker volume in place and never prints row values.

## 1. Freeze and archive

1. Schedule a no-write window and stop both old application stacks.
2. Inspect the complete Garmin and running-shoe volumes, including WAL/SHM and adjacent files. If Garmin records refer to persisted FIT/upload files, migrate those files separately before cutover.
3. Create a private staging directory and restrict it to the operator:

```bash
install -d -m 0700 migration-artifacts/raw migration-artifacts/snapshots migration-artifacts/reports
```

4. Archive each stopped volume through a read-only mount. Use the real volume names for the deployment; do not point the tool at the mounts:

```bash
docker run --rm -v garmin_fit_data:/source:ro alpine:3.22 tar -C /source -cf - . \
  > migration-artifacts/raw/garmin-volume.tar
docker run --rm -v running-shoe-size-advisor-admin-data:/source:ro alpine:3.22 tar -C /source -cf - . \
  > migration-artifacts/raw/running-shoe-intake-volume.tar
sha256sum migration-artifacts/raw/*.tar > migration-artifacts/raw/SHA256SUMS
chmod 0400 migration-artifacts/raw/*.tar migration-artifacts/raw/SHA256SUMS
```

5. Extract the archives into private staging directories. Copy committed WAL-visible SQLite files into those directories. Keep the original archives untouched.

6. Create standalone snapshots. The source names are ledger identities and must remain stable for reruns:

```bash
cargo run -p legacy-data-migrator -- snapshot \
  --source-name garmin \
  --input /private/staging/garmin/source.sqlite3 \
  --output migration-artifacts/snapshots/garmin.sqlite3 \
  --report migration-artifacts/reports/garmin-snapshot.json
cargo run -p legacy-data-migrator -- snapshot \
  --source-name intake \
  --input /private/staging/intake/intakes.sqlite \
  --output migration-artifacts/snapshots/intake.sqlite \
  --report migration-artifacts/reports/intake-snapshot.json
```

Snapshot checks run `quick_check`, `foreign_key_check`, required-schema validation, a consistent read, and a SHA-256 over the resulting snapshot. Do not delete the raw archives or snapshots until the cutover is accepted.

## 2. Prepare PostgreSQL

Create `.env` from `.env.example` with generated credentials. Passwords must be URL-safe or percent-encoded in `DATABASE_URL`; never commit the file.

```bash
install -d -m 0700 db-data
POSTGRES_DB=test POSTGRES_USER=test POSTGRES_PASSWORD=test \
DATABASE_URL=postgresql://test:test@postgres:5432/test ADMIN_EMAILS=admin@example.invalid \
docker compose config
```

Start only PostgreSQL and wait for the Compose healthcheck:

```bash
docker compose up -d postgres
docker compose ps postgres
```

Run the PostgreSQL migration set against a host-reachable URL. The command refuses SQLite URLs and creates the clean baseline, transcript queue, and import ledger:

```bash
cargo run -p legacy-data-migrator -- prepare-target \
  --database-url "$DATABASE_URL"
```

Before importing, save a schema-only dump in the ignored artifact directory:

```bash
pg_dump --schema-only --file=migration-artifacts/reports/pre-import-schema.sql "$DATABASE_URL"
chmod 0400 migration-artifacts/reports/pre-import-schema.sql
```

## 3. Dry run, apply, and verify

Import requires both standalone snapshots. Without `--apply`, all inserts, comparisons, sequence repairs, and ledger work occur in one transaction and roll back:

```bash
cargo run -p legacy-data-migrator -- import \
  --database-url "$DATABASE_URL" \
  --garmin-snapshot migration-artifacts/snapshots/garmin.sqlite3 \
  --intake-snapshot migration-artifacts/snapshots/intake.sqlite \
  --report migration-artifacts/reports/import-dry-run.json
```

Review only counts and digests. The report must include every Garmin table from migrations `0001`–`0003`, empty OAuth transient tables, `transcript_entries`, and no row contents. Investigate any count mismatch; do not edit the importer to suppress it.

After the dry run is accepted, apply the exact same snapshots:

```bash
cargo run -p legacy-data-migrator -- import --apply \
  --database-url "$DATABASE_URL" \
  --garmin-snapshot migration-artifacts/snapshots/garmin.sqlite3 \
  --intake-snapshot migration-artifacts/snapshots/intake.sqlite \
  --report migration-artifacts/reports/import-apply.json
cargo run -p legacy-data-migrator -- verify \
  --database-url "$DATABASE_URL" \
  --garmin-snapshot migration-artifacts/snapshots/garmin.sqlite3 \
  --intake-snapshot migration-artifacts/snapshots/intake.sqlite \
  --report migration-artifacts/reports/import-verify.json
```

Run the same `import --apply` command a second time. It must recognize matching ledger hashes and manifests, perform no data changes, and leave ledger timestamps unchanged. A different snapshot hash for `garmin` or `intake`, an existing row mismatch, a unique-key collision with another primary key, a foreign-key failure, or a sequence discrepancy is a hard failure; the transaction must roll back without overwriting target data.

Take a restricted post-import dump and checksum:

```bash
pg_dump --format=custom --file=migration-artifacts/post-import.dump "$DATABASE_URL"
chmod 0400 migration-artifacts/post-import.dump
sha256sum migration-artifacts/post-import.dump > migration-artifacts/post-import.dump.sha256
```

## 4. Cutover gates

Before allowing mutating traffic:

- `docker compose ps` reports healthy PostgreSQL and application services.
- `/healthz`, `/shoes`, a known shoe detail route, and SPA deep links work.
- Garmin `/`, `/upload`, `/history`, and `/extractions/{id}` retain session behavior.
- Public requests cannot read transcript entries.
- An unauthenticated admin API request returns `401`; an authenticated non-allowlisted user returns `403`.
- The allowlisted operator can list and edit the preserved transcript row, including its historical ID.
- Imported history and extraction details show the expected source counts and associations.
- PostgreSQL uses the intended intranet firewall policy for the published `5432` port.

Only after all gates pass should the unified service receive traffic. Keep the old SQLite applications stopped and their volumes untouched.

## 5. Rollback boundary

Before PostgreSQL accepts new application writes, remove unified traffic, stop the app and PostgreSQL, preserve `db-data` and the PostgreSQL dump, and restore the old services from their untouched configuration and volumes. Diagnose from reports and logs without modifying source backups.

After PostgreSQL accepts new writes, direct rollback to SQLite is unsafe because this repository intentionally has no reverse synchronizer. Use a validated roll-forward repair or design, implement, and verify a separate reverse migration before restoring old services.

Migration artifacts contain sensitive sessions, OAuth values, and transcription data indirectly through database dumps. Keep them outside Git with private permissions and encrypted backup storage. The repository ignores `.env`, database dumps, snapshots, `db-data`, and migration artifacts.
