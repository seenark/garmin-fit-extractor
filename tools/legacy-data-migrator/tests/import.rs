use std::{
    collections::BTreeMap,
    env,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use garmin_fit_extractor_api::runs::{jobs, store};
use legacy_data_migrator::{
    MigratorError,
    source::{Snapshot, SourceKind, read_snapshot, write_snapshot},
    target,
};
use sha2::{Digest, Sha256};
use sqlx::{
    PgPool, Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use uuid::Uuid;

const GARMIN_OWNER: &str = "10000000-0000-4000-8000-000000000001";
const OTHER_OWNER: &str = "10000000-0000-4000-8000-000000000002";
const GARMIN_FIT: &[u8] = include_bytes!("../../../apps/api/tests/fixtures/runs/garmin_run.fit");

fn fixture_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "legacy-migrator-{label}-{}-{nonce}.sqlite3",
        std::process::id()
    ))
}

async fn create_sqlite(path: &PathBuf, kind: SourceKind, owner_id: &str) -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("create fixture database");
    if kind == SourceKind::Garmin {
        for statement in [
            "CREATE TABLE users (id TEXT PRIMARY KEY, google_subject TEXT NOT NULL UNIQUE, email TEXT NOT NULL, display_name TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
            "CREATE TABLE sessions (token_hash TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE, created_at TEXT NOT NULL, expires_at TEXT NOT NULL)",
            "CREATE TABLE oauth_states (state_hash TEXT PRIMARY KEY, nonce TEXT NOT NULL, pkce_verifier TEXT NOT NULL, created_at TEXT NOT NULL, expires_at TEXT NOT NULL, continue_path TEXT)",
            "CREATE TABLE extractions (id TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE, file_name TEXT NOT NULL, file_size_bytes INTEGER NOT NULL, status TEXT NOT NULL, activity_type TEXT, activity_date TEXT, normalized_json TEXT, raw_json TEXT, error_code TEXT, error_message TEXT, created_at TEXT NOT NULL)",
            "CREATE TABLE activities (id TEXT PRIMARY KEY REFERENCES extractions(id) ON DELETE CASCADE, owner_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE, sport TEXT, started_at TEXT NOT NULL, activity_data TEXT NOT NULL, created_at TEXT NOT NULL)",
            "CREATE TABLE oauth_login_requests (request_hash TEXT PRIMARY KEY, client_id TEXT NOT NULL, redirect_uri TEXT NOT NULL, state TEXT NOT NULL, scope TEXT NOT NULL, created_at TEXT NOT NULL, expires_at TEXT NOT NULL)",
            "CREATE TABLE oauth_authorization_codes (code_hash TEXT PRIMARY KEY, client_id TEXT NOT NULL, redirect_uri TEXT NOT NULL, user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE, scope TEXT NOT NULL, created_at TEXT NOT NULL, expires_at TEXT NOT NULL)",
            "CREATE TABLE oauth_access_tokens (token_hash TEXT PRIMARY KEY, client_id TEXT NOT NULL, user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE, scope TEXT NOT NULL, created_at TEXT NOT NULL, expires_at TEXT NOT NULL)",
            "CREATE TABLE oauth_refresh_tokens (token_hash TEXT PRIMARY KEY, client_id TEXT NOT NULL, user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE, scope TEXT NOT NULL, created_at TEXT NOT NULL, expires_at TEXT NOT NULL, revoked_at TEXT)",
        ] {
            sqlx::query(statement)
                .execute(&pool)
                .await
                .expect("create Garmin table");
        }
        sqlx::query("INSERT INTO users VALUES (?, ?, ?, ?, ?, ?)")
            .bind(owner_id)
            .bind("google-subject")
            .bind("runner@example.invalid")
            .bind(Option::<String>::None)
            .bind("2025-01-01T00:00:00.000Z")
            .bind("2025-01-01T00:00:00.000Z")
            .execute(&pool)
            .await
            .expect("insert user");
        sqlx::query("INSERT INTO sessions VALUES (?, ?, ?, ?)")
            .bind("session-hash")
            .bind(owner_id)
            .bind("2025-01-01T00:00:00.000Z")
            .bind("2026-01-01T00:00:00.000Z")
            .execute(&pool)
            .await
            .expect("insert session");
        sqlx::query("INSERT INTO extractions VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind("extraction-uuid-like")
            .bind(owner_id)
            .bind("empty.fit")
            .bind(0_i64)
            .bind("succeeded")
            .bind(Option::<String>::None)
            .bind(Option::<String>::None)
            .bind("{ \"unicode\": \"雪\" }")
            .bind("{\"raw\":true}")
            .bind(Option::<String>::None)
            .bind(Option::<String>::None)
            .bind("2025-01-01T00:00:00.000Z")
            .execute(&pool)
            .await
            .expect("insert extraction");
        sqlx::query("INSERT INTO activities VALUES (?, ?, ?, ?, ?, ?)")
            .bind("extraction-uuid-like")
            .bind(owner_id)
            .bind(Option::<String>::None)
            .bind("2025-01-01T00:00:00.000Z")
            .bind("{ \"raw\": true }")
            .bind("2025-01-01T00:00:00.000Z")
            .execute(&pool)
            .await
            .expect("insert activity");
    } else {
        sqlx::query("CREATE TABLE transcript_entries (id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, channel_name TEXT NOT NULL, youtube_url TEXT NOT NULL, video_id TEXT NOT NULL, transcription TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL)")
            .execute(&pool)
            .await
            .expect("create transcript table");
        sqlx::query("CREATE UNIQUE INDEX transcript_entries_video_id_unique ON transcript_entries(video_id)")
            .execute(&pool)
            .await
            .expect("create transcript unique index");
        sqlx::query("INSERT INTO transcript_entries VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(56_i64)
            .bind("ช่องภาษาไทย")
            .bind("https://example.invalid/watch?v=abc")
            .bind("abc")
            .bind("transcription with unicode 雪 and empty-safe text")
            .bind(1_725_000_000_000_i64)
            .bind(1_725_000_000_001_i64)
            .execute(&pool)
            .await
            .expect("insert transcript");
    }
    pool
}

fn assert_owned_test_database_url(database_url: &str) {
    let parsed = url::Url::parse(database_url).expect("parse explicit TEST_DATABASE_URL");
    let database_name = parsed.path().strip_prefix('/').unwrap_or_default();
    assert!(
        database_name.ends_with("_test") || database_name.ends_with("_rehearsal"),
        "TEST_DATABASE_URL database name must end in _test or _rehearsal"
    );
    let decoder = env::var_os("RUNS_DECODER_EXECUTABLE")
        .expect("RUNS_DECODER_EXECUTABLE must point to the real API decoder binary");
    assert!(
        PathBuf::from(decoder).is_file(),
        "RUNS_DECODER_EXECUTABLE must point to the real API decoder binary"
    );
    assert!(
        env::var_os("PGDATA").is_some_and(|path| !path.is_empty()),
        "PGDATA must identify the disposable PostgreSQL cluster"
    );
}

async fn assert_owned_test_database(pool: &PgPool, database_url: &str) {
    let actual_database: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(pool)
        .await
        .expect("read actual database name");
    let parsed = url::Url::parse(database_url).expect("parse explicit TEST_DATABASE_URL");
    assert_eq!(
        actual_database,
        parsed.path().strip_prefix('/').unwrap_or_default(),
        "connected database must match TEST_DATABASE_URL"
    );
    let actual_directory: String = sqlx::query_scalar("SHOW data_directory")
        .fetch_one(pool)
        .await
        .expect("read actual PostgreSQL data directory");
    let expected_directory = env::var("PGDATA").expect("PGDATA identifies owned cluster");
    assert_eq!(
        std::fs::canonicalize(actual_directory).expect("canonicalize active PostgreSQL directory"),
        std::fs::canonicalize(expected_directory).expect("canonicalize PGDATA"),
        "active PostgreSQL data directory must match PGDATA"
    );
    let version: String = sqlx::query_scalar("SHOW server_version_num")
        .fetch_one(pool)
        .await
        .expect("read PostgreSQL version");
    assert!(
        (180000..190000).contains(&version.parse::<i32>().expect("parse PostgreSQL version")),
        "PostgreSQL 18 is required"
    );
}

async fn protected_fingerprints(pool: &PgPool) -> BTreeMap<String, String> {
    let mut fingerprints = BTreeMap::new();
    for (table, key) in [
        ("users", "id"),
        ("sessions", "token_hash"),
        ("oauth_states", "state_hash"),
        ("extractions", "id"),
        ("oauth_login_requests", "request_hash"),
        ("oauth_authorization_codes", "code_hash"),
        ("oauth_access_tokens", "token_hash"),
        ("oauth_refresh_tokens", "token_hash"),
        ("transcript_entries", "id"),
        ("legacy_imports", "source_name"),
    ] {
        let query = format!(
            "SELECT COALESCE(jsonb_agg(to_jsonb(row_data) ORDER BY row_data.{key}), '[]'::jsonb)::text FROM {table} AS row_data"
        );
        let rows: String = sqlx::query_scalar(sqlx::AssertSqlSafe(query))
            .fetch_one(pool)
            .await
            .expect("fingerprint protected table");
        fingerprints.insert(table.to_owned(), rows);
    }
    let legacy_activities: String = sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(to_jsonb(activity) ORDER BY activity.id), '[]'::jsonb)::text
         FROM activities AS activity
         WHERE EXISTS (SELECT 1 FROM extractions WHERE extractions.id = activity.id)",
    )
    .fetch_one(pool)
    .await
    .expect("fingerprint legacy activity projections");
    fingerprints.insert("legacy_activities".to_owned(), legacy_activities);
    fingerprints
}

async fn native_fingerprint(pool: &PgPool, owners: &[String]) -> String {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT 'source', id, to_jsonb(source)::text
           FROM runs_sources AS source WHERE owner_id = ANY($1::text[])
         UNION ALL
         SELECT 'activity', id, to_jsonb(activity)::text
           FROM runs_activities AS activity WHERE owner_id = ANY($1::text[])
         UNION ALL
         SELECT 'manifest', id, to_jsonb(manifest)::text
           FROM runs_manifests AS manifest WHERE owner_id = ANY($1::text[])
         UNION ALL
         SELECT 'revision', id, to_jsonb(revision)::text
           FROM runs_revisions AS revision WHERE owner_id = ANY($1::text[])
         UNION ALL
         SELECT 'chunk', revision_id || ':' || document || ':' || position::text, to_jsonb(chunk)::text
           FROM runs_revision_chunks AS chunk WHERE owner_id = ANY($1::text[])
         UNION ALL
         SELECT 'job', id, to_jsonb(job)::text
           FROM runs_jobs AS job WHERE owner_id = ANY($1::text[])
         UNION ALL
         SELECT 'estimate', id, to_jsonb(estimate)::text
           FROM runs_estimates AS estimate WHERE owner_id = ANY($1::text[])
         UNION ALL
         SELECT 'estimate_dependency', estimate_id || ':' || activity_id, to_jsonb(dependency)::text
           FROM runs_estimate_dependencies AS dependency WHERE owner_id = ANY($1::text[])
         UNION ALL
         SELECT 'legacy_summary', id, to_jsonb(summary)::text
           FROM runs_legacy_summaries AS summary WHERE owner_id = ANY($1::text[])
         UNION ALL
         SELECT 'import_report', id, to_jsonb(report)::text
           FROM runs_import_reports AS report WHERE owner_id = ANY($1::text[])
         UNION ALL
         SELECT 'tombstone', activity_id, to_jsonb(tombstone)::text
           FROM runs_tombstones AS tombstone WHERE owner_id = ANY($1::text[])
         UNION ALL
         SELECT 'export', token, to_jsonb(export)::text
           FROM runs_exports AS export WHERE owner_id = ANY($1::text[])
         UNION ALL
         SELECT 'export_chunk', chunk.token || ':' || chunk.position::text, to_jsonb(chunk)::text
           FROM runs_export_chunks AS chunk
           JOIN runs_exports AS export ON export.token = chunk.token
          WHERE export.owner_id = ANY($1::text[])
         UNION ALL
         SELECT 'slot', slot_row.stage || ':' || slot_row.slot::text, to_jsonb(slot_row)::text
           FROM runs_slots AS slot_row
         ORDER BY 1, 2",
    )
    .bind(owners)
    .fetch_all(pool)
    .await
    .expect("fingerprint native Runs records");
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&rows).expect("serialize native rows"))
    )
}

async fn import_real_run(pool: &PgPool, owner: Uuid) -> String {
    let admission = jobs::import_admission(pool)
        .await
        .expect("admit native import");
    let mut document = Box::pin(jobs::decode_import(
        pool,
        GARMIN_FIT,
        std::future::pending(),
        Some(
            admission
                .decoder_hold()
                .expect("hold native import capacity"),
        ),
    ))
    .await
    .expect("decode canonical public FIT with API decoder");
    document.protect_cpu(&admission);
    let activity_id = Box::pin(store::accept_spool(
        pool,
        owner,
        GARMIN_FIT,
        document,
        admission.holder(),
    ))
    .await
    .expect("accept source through native API")
    .activity_id;
    drop(admission);
    for _ in 0..8 {
        if !Box::pin(jobs::run_once(pool))
            .await
            .expect("process native durable job")
        {
            return activity_id.to_string();
        }
    }
    panic!("native processing queue did not drain");
}

#[tokio::test]
async fn generated_snapshots_preserve_unicode_nulls_json_and_explicit_id() {
    let garmin_path = fixture_path("garmin");
    let intake_path = fixture_path("intake");
    let garmin_snapshot_path = fixture_path("garmin-snapshot");
    let intake_snapshot_path = fixture_path("intake-snapshot");
    let garmin_pool = create_sqlite(&garmin_path, SourceKind::Garmin, "user-α").await;
    let intake_pool = create_sqlite(&intake_path, SourceKind::Intake, "").await;
    garmin_pool.close().await;
    intake_pool.close().await;

    let garmin = read_snapshot(&garmin_path, SourceKind::Garmin)
        .await
        .expect("read Garmin");
    let intake = read_snapshot(&intake_path, SourceKind::Intake)
        .await
        .expect("read intake");
    let Snapshot::Garmin(garmin) = &garmin else {
        panic!("wrong source kind")
    };
    let Snapshot::Intake(intake) = &intake else {
        panic!("wrong source kind")
    };
    assert_eq!(garmin.users[0].id, "user-α");
    assert_eq!(garmin.users[0].display_name, None);
    assert_eq!(
        garmin.extractions[0].normalized_json.as_deref(),
        Some("{ \"unicode\": \"雪\" }")
    );
    assert_eq!(intake.transcript_entries[0].id, 56);
    assert_eq!(intake.transcript_entries[0].created_at, 1_725_000_000_000);

    let copied_garmin = write_snapshot(&garmin_path, &garmin_snapshot_path, SourceKind::Garmin)
        .await
        .expect("copy Garmin snapshot");
    let copied_intake = write_snapshot(&intake_path, &intake_snapshot_path, SourceKind::Intake)
        .await
        .expect("copy intake snapshot");
    assert_eq!(copied_garmin.source_sha256(), garmin.source_sha256());
    assert_eq!(copied_intake.source_sha256(), intake.source_sha256());

    for path in [
        garmin_path,
        intake_path,
        garmin_snapshot_path,
        intake_snapshot_path,
    ] {
        let _ = std::fs::remove_file(path);
    }
}

#[tokio::test]
async fn postgres_round_trip_runs_only_with_explicit_disposable_database() {
    let database_url = match env::var("TEST_DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("SKIP postgres_round_trip: TEST_DATABASE_URL is not set");
            return;
        }
    };
    assert!(target::ensure_postgres_url(&database_url).is_ok());
    assert_owned_test_database_url(&database_url);
    let identity_pool = target::connect(&database_url)
        .await
        .expect("connect explicit disposable PostgreSQL target");
    assert_owned_test_database(&identity_pool, &database_url).await;
    identity_pool.close().await;

    target::prepare_target(&database_url)
        .await
        .expect("prepare disposable PostgreSQL target");
    let pool = target::connect(&database_url)
        .await
        .expect("connect disposable PostgreSQL target");
    assert_owned_test_database(&pool, &database_url).await;
    sqlx::query("TRUNCATE TABLE users, sessions, oauth_states, extractions, activities, oauth_login_requests, oauth_authorization_codes, oauth_access_tokens, oauth_refresh_tokens, transcript_entries, legacy_imports, runs_sources RESTART IDENTITY CASCADE")
        .execute(&pool)
        .await
        .expect("clear disposable target");
    let sequence_before: (i64, bool) =
        sqlx::query_as("SELECT last_value, is_called FROM public.transcript_entries_id_seq")
            .fetch_one(&pool)
            .await
            .expect("read initial transcript sequence");
    pool.close().await;

    let garmin_path = fixture_path("pg-garmin");
    let intake_path = fixture_path("pg-intake");
    let garmin_snapshot_path = fixture_path("pg-garmin-snapshot");
    let intake_snapshot_path = fixture_path("pg-intake-snapshot");
    let garmin_pool = create_sqlite(&garmin_path, SourceKind::Garmin, GARMIN_OWNER).await;
    let intake_pool = create_sqlite(&intake_path, SourceKind::Intake, "").await;
    sqlx::query("INSERT INTO users VALUES (?, ?, ?, ?, ?, ?)")
        .bind(OTHER_OWNER)
        .bind("google-subject-other")
        .bind("other@example.invalid")
        .bind(Option::<String>::None)
        .bind("2025-01-01T00:00:00.000Z")
        .bind("2025-01-01T00:00:00.000Z")
        .execute(&garmin_pool)
        .await
        .expect("insert second isolated owner");
    garmin_pool.close().await;
    intake_pool.close().await;
    let Snapshot::Garmin(garmin) =
        write_snapshot(&garmin_path, &garmin_snapshot_path, SourceKind::Garmin)
            .await
            .expect("write Garmin snapshot")
    else {
        panic!("wrong Garmin snapshot kind");
    };
    let Snapshot::Intake(intake) =
        write_snapshot(&intake_path, &intake_snapshot_path, SourceKind::Intake)
            .await
            .expect("write intake snapshot")
    else {
        panic!("wrong intake snapshot kind");
    };

    let dry_run = target::import(&database_url, &garmin, &intake, false)
        .await
        .expect("dry-run import");
    assert_eq!(dry_run.status, "dry-run-rolled-back");
    let pool = target::connect(&database_url)
        .await
        .expect("reconnect target");
    let (users, transcript): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM users), (SELECT count(*) FROM transcript_entries)",
    )
    .fetch_one(&pool)
    .await
    .expect("read dry-run counts");
    assert_eq!((users, transcript), (0, 0));
    let sequence_after_dry_run: (i64, bool) =
        sqlx::query_as("SELECT last_value, is_called FROM public.transcript_entries_id_seq")
            .fetch_one(&pool)
            .await
            .expect("read sequence after dry-run");
    assert_eq!(sequence_after_dry_run, sequence_before);
    pool.close().await;

    let applied = target::import(&database_url, &garmin, &intake, true)
        .await
        .expect("apply import");
    assert_eq!(applied.status, "committed");
    let pool = target::connect(&database_url)
        .await
        .expect("reconnect applied target");
    let (users, sessions, extractions, activities, transcript): (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM users), (SELECT count(*) FROM sessions), (SELECT count(*) FROM extractions), (SELECT count(*) FROM activities), (SELECT count(*) FROM transcript_entries)",
    )
    .fetch_one(&pool)
    .await
    .expect("read applied counts");
    assert_eq!(
        (users, sessions, extractions, activities, transcript),
        (2, 1, 1, 1, 1)
    );
    let next_id: i64 = sqlx::query_scalar("SELECT nextval('public.transcript_entries_id_seq')")
        .fetch_one(&pool)
        .await
        .expect("read next transcript ID");
    assert_eq!(next_id, 57);
    let imported_at_before: String = sqlx::query_scalar(
        "SELECT imported_at::text FROM legacy_imports WHERE source_name = 'garmin'",
    )
    .fetch_one(&pool)
    .await
    .expect("read ledger timestamp");
    pool.close().await;

    let reapplied = target::import(&database_url, &garmin, &intake, true)
        .await
        .expect("idempotent apply");
    assert_eq!(reapplied.status, "committed");
    let pool = target::connect(&database_url)
        .await
        .expect("reconnect idempotent target");
    let imported_at_after: String = sqlx::query_scalar(
        "SELECT imported_at::text FROM legacy_imports WHERE source_name = 'garmin'",
    )
    .fetch_one(&pool)
    .await
    .expect("read unchanged ledger timestamp");
    assert_eq!(imported_at_after, imported_at_before);
    pool.close().await;

    let verified = target::verify(&database_url, &garmin, &intake)
        .await
        .expect("verify imported snapshots");
    assert_eq!(verified.status, "verified");

    let owners = vec![GARMIN_OWNER.to_owned(), OTHER_OWNER.to_owned()];
    let pool = target::connect(&database_url)
        .await
        .expect("connect native test target");
    let owner_a = Uuid::parse_str(GARMIN_OWNER).expect("valid Garmin owner UUID");
    let owner_b = Uuid::parse_str(OTHER_OWNER).expect("valid second owner UUID");
    let activity_a = import_real_run(&pool, owner_a).await;
    let activity_b = import_real_run(&pool, owner_b).await;
    let source_rows: Vec<(String, Vec<u8>, i64, Vec<u8>)> = sqlx::query_as(
        "SELECT owner_id, sha256, size_bytes, bytes
         FROM runs_sources WHERE owner_id = ANY($1::text[]) ORDER BY owner_id",
    )
    .bind(&owners)
    .fetch_all(&pool)
    .await
    .expect("read accepted native sources");
    assert_eq!(source_rows.len(), 2);
    assert_eq!(source_rows[0].0, GARMIN_OWNER);
    assert_eq!(source_rows[1].0, OTHER_OWNER);
    for (_, sha256, size_bytes, bytes) in &source_rows {
        assert_eq!(bytes.as_slice(), GARMIN_FIT);
        assert_eq!(*size_bytes, GARMIN_FIT.len() as i64);
        assert_eq!(*sha256, Sha256::digest(bytes).to_vec());
    }
    let native_rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT owner_id, id, processing_status, current_manifest_id
         FROM runs_activities WHERE owner_id = ANY($1::text[]) ORDER BY owner_id",
    )
    .bind(&owners)
    .fetch_all(&pool)
    .await
    .expect("read accepted native activities");
    assert_eq!(native_rows.len(), 2);
    assert_eq!(native_rows[0].0, GARMIN_OWNER);
    assert_eq!(native_rows[0].1, activity_a);
    assert_eq!(native_rows[0].2, "ready");
    assert!(native_rows[0].3.is_some());
    assert_eq!(native_rows[1].0, OTHER_OWNER);
    assert_eq!(native_rows[1].1, activity_b);
    assert_eq!(native_rows[1].2, "ready");
    assert!(native_rows[1].3.is_some());
    let coherent_manifests: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM runs_activities AS activity
         JOIN runs_sources AS source
           ON source.id = activity.source_id AND source.owner_id = activity.owner_id
         JOIN runs_manifests AS manifest
           ON manifest.id = activity.current_manifest_id
          AND manifest.activity_id = activity.id AND manifest.owner_id = activity.owner_id
         WHERE activity.owner_id = ANY($1::text[]) AND activity.processing_status = 'ready'",
    )
    .bind(&owners)
    .fetch_one(&pool)
    .await
    .expect("verify ready native manifests");
    assert_eq!(coherent_manifests, 2);
    let native_before = native_fingerprint(&pool, &owners).await;
    let protected_before = protected_fingerprints(&pool).await;
    let sequence_before_native_repeat: (i64, bool) =
        sqlx::query_as("SELECT last_value, is_called FROM public.transcript_entries_id_seq")
            .fetch_one(&pool)
            .await
            .expect("read sequence before native repeat");
    pool.close().await;

    let native_repeat = target::import(&database_url, &garmin, &intake, true)
        .await
        .expect("reimport legacy snapshots beside native runs");
    assert_eq!(native_repeat.status, "committed");
    let native_verified = target::verify(&database_url, &garmin, &intake)
        .await
        .expect("verify snapshots beside native runs");
    assert_eq!(native_verified.status, "verified");
    let pool = target::connect(&database_url)
        .await
        .expect("reconnect after native legacy verification");
    assert_eq!(native_fingerprint(&pool, &owners).await, native_before);
    assert_eq!(protected_fingerprints(&pool).await, protected_before);
    let sequence_after_native_repeat: (i64, bool) =
        sqlx::query_as("SELECT last_value, is_called FROM public.transcript_entries_id_seq")
            .fetch_one(&pool)
            .await
            .expect("read sequence after native repeat");
    assert_eq!(sequence_after_native_repeat, sequence_before_native_repeat);
    pool.close().await;

    let mut changed = garmin.clone();
    changed.users[0].email = "changed@example.invalid".to_owned();
    let mismatch = target::import(&database_url, &changed, &intake, true)
        .await
        .expect_err("changed source must be refused by ledger");
    assert!(matches!(mismatch, MigratorError::LedgerMismatch { .. }));

    let pool = target::connect(&database_url)
        .await
        .expect("connect native integrity target");
    let native_before_fault = native_fingerprint(&pool, &owners).await;
    let mut native_boundary = garmin.clone();
    let mut rollback_session = native_boundary.sessions[0].clone();
    rollback_session.token_hash = "must-roll-back-session".to_owned();
    native_boundary.sessions.push(rollback_session);
    sqlx::query("DELETE FROM legacy_imports")
        .execute(&pool)
        .await
        .expect("clear ledger for native owner boundary");
    let changed_rows = sqlx::query("UPDATE activities SET owner_id = $1 WHERE id = $2")
        .bind(OTHER_OWNER)
        .bind(&activity_a)
        .execute(&pool)
        .await
        .expect("corrupt native activity projection owner")
        .rows_affected();
    assert_eq!(changed_rows, 1);
    let sequence_before_native_refusal: (i64, bool) =
        sqlx::query_as("SELECT last_value, is_called FROM public.transcript_entries_id_seq")
            .fetch_one(&pool)
            .await
            .expect("read sequence before native refusal");
    let mismatch = target::import(&database_url, &native_boundary, &intake, true)
        .await
        .expect_err("native activity with foreign projection owner must be refused");
    assert!(
        matches!(mismatch, MigratorError::TargetMismatch { table, .. } if table == "activities")
    );
    let after_native_refusal: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM sessions WHERE token_hash = 'must-roll-back-session'),
                (SELECT count(*) FROM legacy_imports)",
    )
    .fetch_one(&pool)
    .await
    .expect("read native refusal rollback state");
    assert_eq!(after_native_refusal, (0, 0));
    let sequence_after_native_refusal: (i64, bool) =
        sqlx::query_as("SELECT last_value, is_called FROM public.transcript_entries_id_seq")
            .fetch_one(&pool)
            .await
            .expect("read sequence after native refusal");
    assert_eq!(
        sequence_after_native_refusal,
        sequence_before_native_refusal
    );
    sqlx::query("UPDATE activities SET owner_id = $1 WHERE id = $2")
        .bind(GARMIN_OWNER)
        .bind(&activity_a)
        .execute(&pool)
        .await
        .expect("restore native activity projection owner");
    pool.close().await;
    let restored = target::import(&database_url, &garmin, &intake, true)
        .await
        .expect("restore legacy ledger after native fault");
    assert_eq!(restored.status, "committed");
    let restored_verify = target::verify(&database_url, &garmin, &intake)
        .await
        .expect("verify restored native boundary");
    assert_eq!(restored_verify.status, "verified");

    let pool = target::connect(&database_url)
        .await
        .expect("connect integrity target");
    assert_eq!(
        native_fingerprint(&pool, &owners).await,
        native_before_fault
    );
    sqlx::query("UPDATE activities SET owner_id = $1 WHERE id = $2")
        .bind(OTHER_OWNER)
        .bind("extraction-uuid-like")
        .execute(&pool)
        .await
        .expect("corrupt legacy activity owner");
    let mut wrong_owner = garmin.clone();
    wrong_owner.activities[0].owner_id = OTHER_OWNER.to_owned();
    let mut new_session = wrong_owner.sessions[0].clone();
    new_session.token_hash = "must-roll-back-session".to_owned();
    wrong_owner.sessions.push(new_session);
    sqlx::query("DELETE FROM legacy_imports")
        .execute(&pool)
        .await
        .expect("clear ledger for ownership boundary");
    let sequence_before_refusal: (i64, bool) =
        sqlx::query_as("SELECT last_value, is_called FROM public.transcript_entries_id_seq")
            .fetch_one(&pool)
            .await
            .expect("read sequence before rejected import");
    let mismatch = target::import(&database_url, &wrong_owner, &intake, true)
        .await
        .expect_err("foreign extraction owner must be refused");
    assert!(
        matches!(mismatch, MigratorError::TargetMismatch { table, .. } if table == "activities")
    );
    let ledgers: i64 = sqlx::query_scalar("SELECT count(*) FROM legacy_imports")
        .fetch_one(&pool)
        .await
        .expect("read rolled-back ledger count");
    assert_eq!(ledgers, 0);
    let rolled_back_sessions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sessions WHERE token_hash = 'must-roll-back-session'",
    )
    .fetch_one(&pool)
    .await
    .expect("read rejected session count");
    assert_eq!(rolled_back_sessions, 0);
    let sequence_after_refusal: (i64, bool) =
        sqlx::query_as("SELECT last_value, is_called FROM public.transcript_entries_id_seq")
            .fetch_one(&pool)
            .await
            .expect("read sequence after rejected import");
    assert_eq!(sequence_after_refusal, sequence_before_refusal);
    sqlx::query("UPDATE activities SET owner_id = $1 WHERE id = $2")
        .bind(GARMIN_OWNER)
        .bind("extraction-uuid-like")
        .execute(&pool)
        .await
        .expect("restore legacy owner");
    sqlx::query("INSERT INTO activities VALUES ('orphan-run',$1,'running','2025-01-01T00:00:00.000Z','{}','2025-01-01T00:00:00.000Z')")
        .bind(GARMIN_OWNER)
        .execute(&pool)
        .await
        .expect("seed genuine orphan activity");
    let mut with_new_session = garmin.clone();
    with_new_session
        .sessions
        .push(wrong_owner.sessions.last().unwrap().clone());
    let orphan = target::import(&database_url, &with_new_session, &intake, true)
        .await
        .expect_err("activity without extraction or native source must be refused");
    assert!(matches!(orphan, MigratorError::TargetMismatch { table, .. } if table == "activities"));
    let after_orphan: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM sessions WHERE token_hash = 'must-roll-back-session'),
                (SELECT count(*) FROM legacy_imports),
                (SELECT count(*) FROM activities WHERE id = 'orphan-run')",
    )
    .fetch_one(&pool)
    .await
    .expect("read atomic orphan refusal");
    assert_eq!(after_orphan, (0, 0, 1));
    sqlx::query("DELETE FROM activities WHERE id = 'orphan-run'")
        .execute(&pool)
        .await
        .expect("remove deliberate orphan");
    pool.close().await;

    for path in [
        garmin_path,
        intake_path,
        garmin_snapshot_path,
        intake_snapshot_path,
    ] {
        let _ = std::fs::remove_file(path);
    }
}

#[allow(dead_code)]
fn _assert_sqlite_row_type(row: &sqlx::sqlite::SqliteRow) -> Option<String> {
    row.try_get("missing").ok()
}
