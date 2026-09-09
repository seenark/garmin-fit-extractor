use std::{
    env,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use legacy_data_migrator::{
    MigratorError,
    source::{Snapshot, SourceKind, read_snapshot, write_snapshot},
    target,
};
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
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

async fn create_sqlite(path: &PathBuf, kind: SourceKind) -> SqlitePool {
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
            .bind("user-α")
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
            .bind("user-α")
            .bind("2025-01-01T00:00:00.000Z")
            .bind("2026-01-01T00:00:00.000Z")
            .execute(&pool)
            .await
            .expect("insert session");
        sqlx::query("INSERT INTO extractions VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind("extraction-uuid-like")
            .bind("user-α")
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
            .bind("user-α")
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

#[tokio::test]
async fn generated_snapshots_preserve_unicode_nulls_json_and_explicit_id() {
    let garmin_path = fixture_path("garmin");
    let intake_path = fixture_path("intake");
    let garmin_snapshot_path = fixture_path("garmin-snapshot");
    let intake_snapshot_path = fixture_path("intake-snapshot");
    let garmin_pool = create_sqlite(&garmin_path, SourceKind::Garmin).await;
    let intake_pool = create_sqlite(&intake_path, SourceKind::Intake).await;
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
    let database_url = env::var("TEST_DATABASE_URL").or_else(|_| env::var("POSTGRES_TEST_URL"));
    let Ok(database_url) = database_url else {
        eprintln!("SKIP postgres_round_trip: TEST_DATABASE_URL or POSTGRES_TEST_URL is not set");
        return;
    };
    assert!(target::ensure_postgres_url(&database_url).is_ok());

    target::prepare_target(&database_url)
        .await
        .expect("prepare disposable PostgreSQL target");
    let pool = target::connect(&database_url)
        .await
        .expect("connect disposable PostgreSQL target");
    sqlx::query("TRUNCATE TABLE users, sessions, oauth_states, extractions, activities, oauth_login_requests, oauth_authorization_codes, oauth_access_tokens, oauth_refresh_tokens, transcript_entries, legacy_imports RESTART IDENTITY CASCADE")
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
    let garmin_pool = create_sqlite(&garmin_path, SourceKind::Garmin).await;
    let intake_pool = create_sqlite(&intake_path, SourceKind::Intake).await;
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
        (1, 1, 1, 1, 1)
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

    let mut changed = garmin.clone();
    changed.users[0].email = "changed@example.invalid".to_owned();
    let mismatch = target::import(&database_url, &changed, &intake, true)
        .await
        .expect_err("changed source must be refused by ledger");
    assert!(matches!(mismatch, MigratorError::LedgerMismatch { .. }));

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
